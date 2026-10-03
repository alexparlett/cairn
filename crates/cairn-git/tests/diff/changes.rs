//! C5: the changes query against git's own detection, under the same configuration — and
//! against what the user's own `git log` shows under their configuration, which is what
//! decision E promises. Since `git diff-tree` answers the query, what these decide is that
//! Cairn asks git the right question and reads the answer whole: the detection the
//! configuration names, the limit git applies, the commits compared, and every record.

use cairn_git::{CancelSignal, Error};
use cairn_git::{ChangeSet, ChangesRequest, Repository};
use cairn_model::{ChangeStatus, ChangedFile, FileMode, Oid};

use super::repositories::{self, Repo};
use super::{git, ok, some};

/// One changed path, spelled the way `git diff-tree --raw` spells one, so both sides of
/// the comparison are the same shape.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Row {
    old_mode: String,
    new_mode: String,
    old_id: String,
    new_id: String,
    status: String,
    old_path: String,
    new_path: String,
}

const ABSENT_MODE: &str = "000000";

fn null_id(width: usize) -> String {
    "0".repeat(width)
}

/// `git diff-tree -r --raw <flags> <revs>`, parsed.
fn git_rows(repo: &Repo, flags: &[&str], revs: &[&str]) -> Vec<Row> {
    let mut args = vec!["diff-tree", "-r", "--raw", "--no-abbrev", "--no-ext-diff"];
    args.extend_from_slice(flags);
    args.extend_from_slice(revs);
    rows_of(&repo.git(&args))
}

/// What the user's own `git log` shows for one commit — porcelain, so it reads
/// `diff.renames` and `diff.renameLimit` itself — with git's stderr, where its rename-limit
/// warning is. The fixtures' paths need no quoting, so the plain `--raw` form is enough.
/// A merge is shown against its first parent with `-m --first-parent`, which every git
/// from the 2.30 floor reads that way; `--diff-merges=first-parent` would say the same
/// from git 2.31 only (both checked against 2.30, 2.32 and 2.56, on a merge, under
/// `log.diffMerges=separate` too).
fn shown(repo: &Repo, commit: &str) -> (Vec<Row>, String) {
    let (status, stdout, stderr) = repo.run(
        &[
            "log",
            "-1",
            "--raw",
            "--no-abbrev",
            "--format=",
            "-m",
            "--first-parent",
            commit,
        ],
        &[],
        None,
    );
    assert!(status.success(), "git log failed: {stderr}");
    (rows_of(&stdout), stderr)
}

/// The limit git's warning asks the user to raise `diff.renameLimit` to, when it printed
/// one — read in the C locale, as a test's oracle and never by the product.
fn warned_limit(stderr: &str) -> Option<usize> {
    if !stderr.contains("rename detection was skipped") {
        return None;
    }
    let after = some(
        stderr.split("at least ").nth(1),
        "the warning names a limit",
    );
    let digits: String = after.chars().take_while(char::is_ascii_digit).collect();
    Some(ok(digits.parse(), "the warning's limit is a number"))
}

/// Raw lines parsed; lines that do not start with `:` (the commit-id header git prints
/// when it is given one commit) are skipped.
fn rows_of(out: &str) -> Vec<Row> {
    let mut rows: Vec<Row> = out
        .lines()
        .filter_map(|line| line.strip_prefix(':'))
        .map(|line| {
            let (meta, paths) = some(line.split_once('\t'), "a raw line has a tab");
            let mut fields = meta.split_whitespace();
            let mut next = || some(fields.next(), "a raw line has five fields").to_owned();
            let (old_mode, new_mode, old_id, new_id, status) =
                (next(), next(), next(), next(), next());
            let mut names = paths.split('\t');
            let first = some(names.next(), "a raw line names a path").to_owned();
            let second = names.next().map(str::to_owned);
            let (old_path, new_path) = match second {
                Some(destination) => (first, destination),
                None => (first.clone(), first),
            };
            Row {
                old_mode,
                new_mode,
                old_id,
                new_id,
                status,
                old_path,
                new_path,
            }
        })
        .collect();
    rows.sort();
    rows
}

fn cairn_rows(files: &[ChangedFile], width: usize) -> Vec<Row> {
    let mut rows: Vec<Row> = files
        .iter()
        .map(|file| Row {
            old_mode: file
                .old_mode
                .map_or_else(|| ABSENT_MODE.to_owned(), |mode| mode.octal().to_owned()),
            new_mode: file
                .new_mode
                .map_or_else(|| ABSENT_MODE.to_owned(), |mode| mode.octal().to_owned()),
            old_id: file
                .old_id
                .map_or_else(|| null_id(width), |id| id.hex().as_str().to_owned()),
            new_id: file
                .new_id
                .map_or_else(|| null_id(width), |id| id.hex().as_str().to_owned()),
            status: match file.status {
                ChangeStatus::Added => "A".to_owned(),
                ChangeStatus::Deleted => "D".to_owned(),
                ChangeStatus::Modified => "M".to_owned(),
                ChangeStatus::TypeChanged => "T".to_owned(),
                ChangeStatus::Renamed(similarity) => format!("R{:03}", similarity.percent()),
                ChangeStatus::Copied(similarity) => format!("C{:03}", similarity.percent()),
            },
            old_path: file.old_path.display().into_owned(),
            new_path: file.new_path.display().into_owned(),
        })
        .collect();
    rows.sort();
    rows
}

/// One query on a freshly opened repository, so it reads the configuration as it is now.
fn changes_of(repo: &Repo, request: &ChangesRequest) -> ChangeSet {
    ok(try_changes(repo, request), "the changes query answers")
}

fn try_changes(repo: &Repo, request: &ChangesRequest) -> Result<ChangeSet, Error> {
    let engine = ok(Repository::discover(repo.path()), "the fixture opens");
    engine.changes(git(), request, &CancelSignal::new())
}

fn hex(repo: &Repo, spec: &str) -> String {
    repo.git(&["rev-parse", spec]).trim().to_owned()
}

/// Every commit of the crafted history, against git's own file list under git's defaults.
/// Statuses, both modes, both ids and rename pairs all have to agree, path for path.
#[test]
fn every_crafted_commit_lists_what_git_lists() {
    let repo = repositories::crafted();
    let commits: Vec<String> = repo
        .git(&["rev-list", "--reverse", "HEAD"])
        .lines()
        .map(str::to_owned)
        .collect();
    assert!(commits.len() >= 8, "the fixture lost commits: {commits:?}");

    for commit in &commits {
        let id = Oid::parse(commit).expect("an id");
        let found = changes_of(&repo, &ChangesRequest::commit(id));
        // `--root` is what makes git diff the first commit against the empty tree, which
        // is what decision L5 says Cairn does without being asked.
        let expected = git_rows(&repo, &["-M", "--root"], &[commit]);
        assert_eq!(
            cairn_rows(&found.files, commit.len()),
            expected,
            "commit {commit} ({}) disagrees with git",
            repo.git(&["log", "-1", "--format=%s", commit]).trim()
        );
        assert!(
            found.details.is_some(),
            "a single commit's changes carry its details (R2.1)"
        );
    }
}

/// The root commit on its own, since it is the case a `<commit>^` spelling cannot even
/// name and the one a diff against nothing is easy to answer as nothing.
#[test]
fn a_root_commit_is_compared_with_the_empty_tree() {
    let repo = repositories::crafted();
    let root = repo
        .git(&["rev-list", "--max-parents=0", "HEAD"])
        .trim()
        .to_owned();
    assert!(
        repo.try_git(&["rev-parse", &format!("{root}^")], &[], None)
            .is_err(),
        "{root} is not a root commit, so this test proves nothing"
    );

    let found = changes_of(
        &repo,
        &ChangesRequest::commit(Oid::parse(&root).expect("an id")),
    );
    assert_eq!(
        cairn_rows(&found.files, root.len()),
        git_rows(&repo, &["-M", "--root"], &[&root])
    );
    assert!(
        found
            .files
            .iter()
            .all(|file| matches!(file.status, ChangeStatus::Added)),
        "against the empty tree every path is an addition: {:?}",
        found.files
    );
}

/// R2.1's merge rule: the first parent, like any other commit, and never a combined diff.
#[test]
fn a_merge_is_compared_with_its_first_parent() {
    let repo = repositories::merged();
    let merge = hex(&repo, "HEAD");
    let first = hex(&repo, "HEAD^1");
    let second = hex(&repo, "HEAD^2");
    assert_ne!(first, second, "the fixture's merge has one parent");

    let found = changes_of(
        &repo,
        &ChangesRequest::commit(Oid::parse(&merge).expect("an id")),
    );
    assert_eq!(
        cairn_rows(&found.files, merge.len()),
        git_rows(&repo, &["-M"], &[&first, &merge]),
        "a merge's diff is against its FIRST parent"
    );
    assert_ne!(
        cairn_rows(&found.files, merge.len()),
        git_rows(&repo, &["-M"], &[&second, &merge]),
        "the two parents give the same answer, so this fixture cannot tell them apart"
    );
    let details = found.details.expect("a commit's details");
    assert!(details.is_merge(), "the subject is a merge");
    assert_eq!(details.parents.len(), 2);
}

/// Two commits, tip against tip, with no commit details because there is no one commit
/// the answer is about (R7.3).
#[test]
fn a_comparison_of_two_commits_is_tip_against_tip() {
    let repo = repositories::crafted();
    let old = hex(&repo, "HEAD~5");
    let new = hex(&repo, "HEAD");

    let found = changes_of(
        &repo,
        &ChangesRequest::between(
            Oid::parse(&old).expect("an id"),
            Oid::parse(&new).expect("an id"),
        ),
    );
    assert_eq!(
        cairn_rows(&found.files, new.len()),
        git_rows(&repo, &["-M"], &[&old, &new])
    );
    assert!(
        found.details.is_none(),
        "a comparison is about no single commit (R7.3)"
    );

    let reversed = changes_of(
        &repo,
        &ChangesRequest::between(
            Oid::parse(&new).expect("an id"),
            Oid::parse(&old).expect("an id"),
        ),
    );
    assert_eq!(
        cairn_rows(&reversed.files, new.len()),
        git_rows(&repo, &["-M"], &[&new, &old]),
        "swapping the pair swaps the diff (R7.2)"
    );
}

/// R2.2 through the config, not through an argument: the same commit read three ways.
#[test]
fn rename_and_copy_detection_follow_the_users_configuration() {
    let off = repositories::rewrites(&[("diff.renames", "false")]);
    let rename = hex(&off, "HEAD~1");
    let found = changes_of(
        &off,
        &ChangesRequest::commit(Oid::parse(&rename).expect("an id")),
    );
    assert_eq!(
        cairn_rows(&found.files, rename.len()),
        git_rows(&off, &["--no-renames"], &[&rename]),
        "with diff.renames off, a rename is a delete and an add"
    );
    assert_eq!(
        found.renames,
        cairn_git::RenameDetection::default(),
        "detection was off"
    );
    assert!(
        found
            .files
            .iter()
            .all(|file| !file.is_rename() && !file.is_copy()),
        "no pair may be reported when detection is off"
    );

    let on = repositories::rewrites(&[("diff.renames", "true")]);
    let rename = hex(&on, "HEAD~1");
    let found = changes_of(
        &on,
        &ChangesRequest::commit(Oid::parse(&rename).expect("an id")),
    );
    assert_eq!(
        cairn_rows(&found.files, rename.len()),
        git_rows(&on, &["-M"], &[&rename])
    );
    assert!(
        found.files.iter().filter(|file| file.is_rename()).count() >= 4,
        "the fixture moves four files: {:?}",
        found.files
    );
    assert!(found.renames.enabled && !found.renames.copies);
    assert!(!found.renames.was_cut_short());
    let default = if git().version() >= since(33) {
        1000
    } else {
        400
    };
    assert_eq!(found.renames.limit, Some(default), "git's own default");

    let copies = repositories::rewrites(&[("diff.renames", "copies")]);
    let copy = hex(&copies, "HEAD");
    let found = changes_of(
        &copies,
        &ChangesRequest::commit(Oid::parse(&copy).expect("an id")),
    );
    assert_eq!(
        cairn_rows(&found.files, copy.len()),
        git_rows(&copies, &["-C"], &[&copy]),
        "with diff.renames=copies, a copy is a copy"
    );
    assert!(
        found.files.iter().any(ChangedFile::is_copy),
        "the fixture copies a file: {:?}",
        found.files
    );
    assert!(found.renames.copies, "{:?}", found.renames);
}

/// R2.2's second half: when `diff.renameLimit` stops the search, the answer says so — and
/// says so exactly when git's own `git log` warns that it did, naming the limit git's
/// warning names. Over renames and copies, a limit either side of each boundary, the stage
/// that pairs exact renames ahead of the limit and the one that pairs by name ahead of it.
/// The `dissimilar` cases are where the count meets the square exactly: two unpaired
/// deletions by two unpaired additions at a limit of 2 is a search git ran and paired
/// nothing in, without a warning, and the same commit at a limit of 1 is one it skipped.
/// Caught by: reading the limit from the wrong config, counting the sources the exact
/// stage paired on a git that culls them (`exact and inexact` at a limit of 2), `>=` for
/// `>` (`dissimilar` at a limit of 2), or an answer that differs from git's at any of
/// these.
#[test]
fn a_rename_limit_that_cuts_detection_short_is_reported_exactly_when_git_warns() {
    type Fixture = fn(&[(&str, &str)]) -> Repo;
    let limits: Fixture = repositories::limits;
    let dissimilar: Fixture = repositories::dissimilar;
    let cases: &[(&str, Fixture, &str, &str, &str)] = &[
        ("limits", limits, "true", "1", "HEAD~1"),
        ("limits", limits, "true", "2", "HEAD~1"),
        ("limits", limits, "true", "3", "HEAD~1"),
        ("limits", limits, "true", "1", "HEAD"),
        ("limits", limits, "copies", "3", "HEAD~1"),
        ("limits", limits, "copies", "4", "HEAD~1"),
        ("limits", limits, "copies", "1", "HEAD"),
        ("limits", limits, "copies", "2", "HEAD"),
        ("dissimilar", dissimilar, "true", "2", "HEAD"),
        ("dissimilar", dissimilar, "true", "1", "HEAD"),
        ("dissimilar", dissimilar, "copies", "2", "HEAD"),
        ("dissimilar", dissimilar, "copies", "1", "HEAD"),
    ];
    let mut cut = 0;
    let mut at_the_square = 0;
    for (name, fixture, renames, limit, commit) in cases {
        let repo = fixture(&[("diff.renames", renames), ("diff.renameLimit", limit)]);
        let id = hex(&repo, commit);
        let found = changes_of(
            &repo,
            &ChangesRequest::commit(Oid::parse(&id).expect("an id")),
        );
        let (rows, stderr) = shown(&repo, &id);
        let case = format!("{name}: diff.renames={renames} diff.renameLimit={limit} on {commit}");
        assert_eq!(cairn_rows(&found.files, id.len()), rows, "{case}");
        let count = |status: &str| rows.iter().filter(|row| row.status == status).count();
        let limit_value: usize = limit.parse().expect("a limit");
        // Only where nothing was paired is the answer's count git's on every version: git
        // 2.30 also counts the deletions its exact stage paired.
        let unpaired = count("D") * count("A");
        if *name == "dissimilar" && *renames == "true" && unpaired == limit_value * limit_value {
            at_the_square += 1;
            assert!(
                !found.renames.was_cut_short(),
                "{case}: {:?}",
                found.renames
            );
        }
        assert_eq!(
            found.renames.needed_limit,
            warned_limit(&stderr),
            "{case}: git said {stderr:?}, Cairn said {:?}",
            found.renames
        );
        assert_eq!(
            found.renames.limit,
            Some(limit.parse().expect("a limit")),
            "{case}"
        );
        cut += usize::from(found.renames.was_cut_short());
    }
    assert!(
        (1..cases.len()).contains(&cut),
        "every case or none was cut short ({cut}), so the boundary was never crossed"
    );
    assert!(
        at_the_square >= 1,
        "no case met the square exactly, so `>` and `>=` are not told apart"
    );
}

/// Decision E's promise, read through the configuration: for each way a user can spell
/// `diff.renames` and `diff.renameLimit` — unset, the words, numbers git reads as booleans,
/// the bare key, unit suffixes and bases — the files the query lists are the ones the
/// user's own `git log` shows, commit by commit, root included. Caught by: plumbing's
/// defaults standing in for the user's (no renames at all), a parser that differs from
/// git's on any spelling, or the bare key read as unset or false.
#[test]
fn the_answer_is_what_git_log_shows_under_each_configuration() {
    let renames: &[Option<&str>] = &[
        None,
        Some("false"),
        Some("true"),
        Some("copies"),
        Some("Copy"),
        Some("yes"),
        Some("off"),
        Some("0"),
        Some("2"),
        Some(""),
        Some("BARE"),
    ];
    let limits: &[Option<&str>] = &[
        None,
        Some("1"),
        Some("0"),
        Some("-1"),
        Some("1k"),
        Some("0x1"),
    ];
    let mut compared = 0;
    for value in renames {
        let repo = repositories::rewrites(&[]);
        match value {
            None => {}
            Some("BARE") => append_config(&repo, "[diff]\n\trenames\n"),
            Some(value) => repo.config("diff.renames", value),
        }
        for limit in limits {
            // Each query opens the repository afresh, so it reads the config as set here.
            let _ = repo.try_git(&["config", "--unset-all", "diff.renameLimit"], &[], None);
            if let Some(limit) = limit {
                repo.config("diff.renameLimit", limit);
            }
            for commit in ["HEAD~2", "HEAD~1", "HEAD"] {
                let id = hex(&repo, commit);
                let found = changes_of(
                    &repo,
                    &ChangesRequest::commit(Oid::parse(&id).expect("an id")),
                );
                let (rows, stderr) = shown(&repo, &id);
                assert_eq!(
                    cairn_rows(&found.files, id.len()),
                    rows,
                    "diff.renames={value:?} diff.renameLimit={limit:?} on {commit}"
                );
                assert_eq!(found.renames.needed_limit, warned_limit(&stderr));
                compared += 1;
            }
        }
    }
    assert_eq!(compared, renames.len() * limits.len() * 3);
}

fn append_config(repo: &Repo, text: &str) {
    use std::io::Write;
    let path = repo.path().join(".git/config");
    let mut file = ok(
        std::fs::OpenOptions::new().append(true).open(&path),
        "the repository's config opens",
    );
    ok(file.write_all(text.as_bytes()), "the config is written");
}

/// A value git refuses is refused here too, rather than read as something it is not: the
/// user's own `git log` fails on it, so there is no answer of git's to show.
#[test]
fn a_configuration_git_refuses_is_refused() {
    for (key, value) in [
        ("diff.renames", "maybe"),
        ("diff.renames", "1x"),
        ("diff.renameLimit", "many"),
        ("diff.renameLimit", "4294967296"),
        ("log.showRoot", "bogus"),
        ("diff.ignoreSubmodules", "ALL"),
        ("diff.ignoreSubmodules", "some"),
    ] {
        // Set once the history is built: `git commit` refuses the value too.
        let repo = repositories::rewrites(&[]);
        let id = hex(&repo, "HEAD~1");
        repo.config(key, value);
        let (status, _, stderr) = repo.run(&["log", "-1", "--raw", "--format=", &id], &[], None);
        assert!(
            !status.success(),
            "git accepts {key}={value}, so this case decides nothing: {stderr}"
        );
        let refused = try_changes(
            &repo,
            &ChangesRequest::commit(Oid::parse(&id).expect("an id")),
        );
        assert!(
            matches!(&refused, Err(Error::InvalidConfig { key: k, .. }) if k == key),
            "{key}={value}: {refused:?}"
        );
    }
}

/// R2.1's order, which a view depends on and gix does not provide: the same query twice
/// gives the same list, and the key is total enough to place a rename pair.
#[test]
fn the_file_list_is_sorted_by_path_and_never_shuffles() {
    let repo = repositories::rewrites(&[("diff.renames", "true")]);
    let rename = Oid::parse(&hex(&repo, "HEAD~1")).expect("an id");
    let first = changes_of(&repo, &ChangesRequest::commit(rename));
    let second = changes_of(&repo, &ChangesRequest::commit(rename));
    assert_eq!(
        first.files, second.files,
        "two runs listed different orders"
    );

    let paths: Vec<_> = first.files.iter().map(|file| &file.new_path).collect();
    let mut sorted = paths.clone();
    sorted.sort();
    assert_eq!(paths, sorted, "the list is not in destination-path order");
    assert!(
        paths.windows(2).all(|pair| pair[0] < pair[1]),
        "two files share a destination path, so the order is not total: {paths:?}"
    );
}

/// R2.9 from outside the crate: a query already superseded answers that it was cancelled
/// and starts no process. Superseding one while `git` runs, and the process ending, is
/// `a_changes_query_superseded_by_a_newer_epoch_stops_git_and_reports_it` in
/// `crates/cairn-git/src/reads/mod.rs`, where the command log can be read.
#[test]
fn a_cancelled_changes_query_answers_cancelled() {
    let repo = repositories::rewrites(&[]);
    let shared = cairn_git::SharedRepository::discover(repo.path()).expect("the fixture opens");
    let engine = shared.to_worker();
    let seed = Oid::parse(&hex(&repo, "HEAD~2")).expect("an id");
    let cancelled = CancelSignal::new();
    cancelled.cancel();
    let error = engine
        .changes(git(), &ChangesRequest::commit(seed), &cancelled)
        .unwrap_err();
    assert!(
        matches!(error, Error::ChangesCancelled { changed: 0 }),
        "a cancelled query must say so: {error:?}"
    );
    assert!(shared.command_log().is_empty(), "git was started anyway");
}

/// R2.10, and the failure a view has to draw something for: on either side of a
/// comparison, and before any process starts.
#[test]
fn a_commit_that_is_not_there_is_refused() {
    let repo = repositories::crafted();
    let shared = cairn_git::SharedRepository::discover(repo.path()).expect("the fixture opens");
    let engine = shared.to_worker();
    let missing = Oid::parse("1234567890abcdef1234567890abcdef12345678").expect("an id");
    let head = Oid::parse(&hex(&repo, "HEAD")).expect("an id");
    for request in [
        ChangesRequest::commit(missing),
        ChangesRequest::between(missing, head),
        ChangesRequest::between(head, missing),
    ] {
        let error = engine
            .changes(git(), &request, &CancelSignal::new())
            .unwrap_err();
        assert!(matches!(error, Error::ReadCommit { .. }), "got {error:?}");
    }
    assert!(
        shared.command_log().is_empty(),
        "git was started for a missing commit"
    );
}

/// R1.8 and R5.3: every field the Commit tab draws, against what git prints for the same
/// commit — the committer beside the author, both offsets, and the whole message.
#[test]
fn the_commit_details_are_the_ones_git_records() {
    let repo = repositories::signed_by_two_people();
    let head = hex(&repo, "HEAD");
    let found = changes_of(
        &repo,
        &ChangesRequest::commit(Oid::parse(&head).expect("an id")),
    );
    let details = found.details.expect("a commit's details");

    let field = |format: &str| {
        repo.git(&["log", "-1", &format!("--format={format}"), &head])
            .trim_end_matches('\n')
            .to_owned()
    };
    assert_eq!(details.id.hex().as_str(), head);
    assert_eq!(details.author.name, field("%an"));
    assert_eq!(details.author.email, field("%ae"));
    assert_eq!(details.author.time.seconds.to_string(), field("%at"));
    assert_eq!(details.author.time.offset(), field("%ai")[20..].to_owned());
    assert_eq!(details.committer.name, field("%cn"));
    assert_eq!(details.committer.email, field("%ce"));
    assert_eq!(details.committer.time.seconds.to_string(), field("%ct"));
    assert_eq!(
        details.committer.time.offset(),
        field("%ci")[20..].to_owned()
    );
    assert_ne!(
        details.author, details.committer,
        "the fixture exists to tell the two apart"
    );
    assert_eq!(
        details.message.trim_end_matches('\n'),
        field("%B").trim_end_matches('\n'),
        "the whole message, not its subject"
    );
    assert!(
        details.body().contains("third paragraph"),
        "the body lost a paragraph: {:?}",
        details.body()
    );
    assert_eq!(
        details.parents,
        vec![Oid::parse(&hex(&repo, "HEAD^")).expect("an id")]
    );

    // Same commit, asked for on its own.
    let engine = Repository::discover(repo.path()).expect("the fixture opens");
    assert_eq!(
        engine.commit_details(&details.id).expect("the details"),
        details,
        "the details a changes query returns must be the details the query for them returns"
    );
}

/// The mode a file mode maps to is the one git spells, which the comparison above rests on.
#[test]
fn every_mode_git_records_reaches_the_model() {
    let repo = repositories::crafted();
    let head = hex(&repo, "HEAD");
    let found = changes_of(
        &repo,
        &ChangesRequest::commit(Oid::parse(&head).expect("an id")),
    );
    let modes: Vec<Option<FileMode>> = found.files.iter().map(|file| file.new_mode).collect();
    assert!(
        modes.contains(&Some(FileMode::Symlink)),
        "the last commit turns a file into a symlink: {:?}",
        found.files
    );
    assert!(
        found
            .files
            .iter()
            .any(|file| matches!(file.status, ChangeStatus::TypeChanged)),
        "a file that became a symlink is a type change: {:?}",
        found.files
    );
}

/// Every file under `dir`, by path, with its bytes.
fn snapshot(dir: &std::path::Path) -> std::collections::BTreeMap<std::path::PathBuf, Vec<u8>> {
    let mut files = std::collections::BTreeMap::new();
    let mut pending = vec![dir.to_owned()];
    while let Some(next) = pending.pop() {
        for entry in ok(std::fs::read_dir(&next), "a directory reads") {
            let path = ok(entry, "an entry").path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let bytes = ok(std::fs::read(&path), "a file reads");
                files.insert(path, bytes);
            }
        }
    }
    files
}

/// The query writes nothing: not the index — though the working tree is stat-dirty, so a
/// refresh would rewrite it — not a ref, not an object, and not the textconv cache a
/// `diff.<driver>.cachetextconv` would fill. The whole git directory is byte-identical
/// afterwards, under copies, a rename search and a root commit, and the configured
/// textconv program never ran. What this decides is the outcome, not the command: between
/// two commits porcelain `git diff --raw` and `git log --raw`, and `--textconv` without a
/// patch, write nothing here either, so which command and flags the query runs is pinned
/// on its argument vector instead (`the_query_is_diff_tree_and_never_runs_a_program`, in
/// `crates/cairn-git/src/reads/changes.rs`). Caught by: a query that does write during
/// the read — a patch under `--textconv` (the notes ref and its objects), or one that
/// started the configured textconv program.
#[test]
fn the_changes_query_writes_nothing() {
    let repo = repositories::attributes();
    repo.config("diff.renames", "copies");
    repo.config("diff.trap.cachetextconv", "true");
    // A copy and a rename for the search to find, beside the driver's files: the copy's
    // source is edited in the same commit, which is where `-C` looks for one.
    let body: String = (0..30)
        .map(|n| format!("line {n} of the source\n"))
        .collect();
    repo.write("source.txt", body.as_bytes());
    repo.commit("a source");
    repo.write("source-copy.txt", body.as_bytes());
    repo.write("source.txt", format!("{body}edited\n").as_bytes());
    repo.git(&["mv", "plain.txt", "moved.txt"]);
    repo.commit("copy and rename");
    // Stat-dirty: same bytes, a new mtime, so a refresh would have something to record.
    std::thread::sleep(std::time::Duration::from_millis(20));
    repo.write("trap.txt", b"watched\nby a program, changed\n");
    let before = snapshot(&repo.path().join(".git"));

    let head = Oid::parse(&hex(&repo, "HEAD")).expect("an id");
    let root = Oid::parse(&hex(&repo, "HEAD~3")).expect("an id");
    let found = changes_of(&repo, &ChangesRequest::commit(head));
    assert!(
        found.files.iter().any(ChangedFile::is_copy)
            && found.files.iter().any(ChangedFile::is_rename),
        "the search found nothing to write about: {:?}",
        found.files
    );
    let _ = changes_of(&repo, &ChangesRequest::commit(root));
    let _ = changes_of(&repo, &ChangesRequest::between(root, head));

    assert_eq!(
        snapshot(&repo.path().join(".git")),
        before,
        "the git directory changed under a read"
    );
    assert!(
        !repo.path().join(".git/refs/notes").exists(),
        "a textconv cache was written"
    );
    assert!(!repositories::trap_ran(&repo), "the textconv program ran");
    repositories::run_trap(&repo);
    assert!(
        repositories::trap_ran(&repo),
        "the trap cannot run at all, so its not running above decides nothing"
    );
}

/// The packs under a repository's object store, by name: a lazy fetch adds one.
fn packs(git_dir: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(git_dir.join("objects/pack"))
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

/// In a blob-less partial clone a rename search needs blobs only the promisor holds, and a
/// read never fetches them (`crate::reads`): the query fails, as `git` fails, and no pack is
/// written — where the user's own `git log` would fetch and show the pairs. With detection
/// off, nothing but trees is read, and the same clone answers. Git older than 2.44 ignores
/// `GIT_NO_LAZY_FETCH` and may fetch; there this says so and decides nothing, unless CI
/// demands a git that honours it.
#[test]
fn in_a_partial_clone_a_rename_search_fails_rather_than_fetching() {
    let honoured = cairn_git::ops::GitVersion {
        major: 2,
        minor: 44,
        patch: 0,
    };
    if git().version() < honoured {
        assert!(
            std::env::var_os("CAIRN_REQUIRE_NO_LAZY_FETCH").is_none(),
            "git {} ignores GIT_NO_LAZY_FETCH (it needs 2.44)",
            git().version()
        );
        eprintln!(
            "SKIPPED in_a_partial_clone_a_rename_search_fails_rather_than_fetching: git {} \
             ignores GIT_NO_LAZY_FETCH",
            git().version()
        );
        return;
    }
    let source = repositories::limits(&[]);
    source.config("uploadpack.allowFilter", "true");
    let holder = Repo::new("partial-holder");
    let clone_path = holder.path().join("clone");
    let url = format!("file://{}", source.path().display());
    holder.git(&[
        "clone",
        "-q",
        "--no-checkout",
        "--filter=blob:none",
        &url,
        &clone_path.to_string_lossy(),
    ]);
    let clone = Repo::borrowed(&clone_path);
    let id = Oid::parse(&hex(&clone, "HEAD~1")).expect("an id");
    let before = packs(&clone_path.join(".git"));

    let searched = try_changes(&clone, &ChangesRequest::commit(id));
    assert!(
        matches!(searched, Err(Error::GitFailed { .. })),
        "a rename search over blobs the clone lacks did not fail: {searched:?}"
    );
    assert_eq!(packs(&clone_path.join(".git")), before, "the read fetched");

    clone.config("diff.renames", "false");
    let listed = changes_of(&clone, &ChangesRequest::commit(id));
    assert_eq!(
        cairn_rows(&listed.files, 40),
        git_rows(&clone, &["--no-renames"], &[&id.to_string()]),
    );
    assert_eq!(packs(&clone_path.join(".git")), before, "the read fetched");
}

/// A shallow clone keeps its boundary commit's parent ids and drops the parents. git's
/// own `git log` reads the shallow file and shows that commit as a root, with no
/// parents at all; so does the query — its file list and its details' parents, which
/// the separate details query agrees with — where comparing with the parent named in
/// the commit would fail on an object the clone does not have. The commit above the
/// boundary is compared as usual and keeps its parent. Caught by: reading the first
/// parent, or the parents, from the commit alone.
#[test]
fn a_shallow_clones_boundary_commit_is_compared_as_git_log_shows_it() {
    let source = repositories::crafted();
    let holder = Repo::new("shallow-holder");
    let clone_path = holder.path().join("clone");
    let url = format!("file://{}", source.path().display());
    holder.git(&[
        "clone",
        "-q",
        "--depth",
        "2",
        &url,
        &clone_path.to_string_lossy(),
    ]);
    let clone = Repo::borrowed(&clone_path);
    for commit in ["HEAD~1", "HEAD"] {
        let id = hex(&clone, commit);
        let found = changes_of(
            &clone,
            &ChangesRequest::commit(Oid::parse(&id).expect("an id")),
        );
        let (rows, _) = shown(&clone, &id);
        assert_eq!(cairn_rows(&found.files, id.len()), rows, "{commit}");
        let parents = some(found.details, "a commit's details").parents;
        let shown_parents: Vec<Oid> = clone
            .git(&["log", "-1", "--format=%P", &id])
            .split_whitespace()
            .map(|parent| Oid::parse(parent).expect("an id"))
            .collect();
        assert_eq!(
            parents, shown_parents,
            "{commit}'s parents, as git log shows them"
        );
        let engine = ok(Repository::discover(clone.path()), "the clone opens");
        assert_eq!(
            ok(
                engine.commit_details(&Oid::parse(&id).expect("an id")),
                "details"
            )
            .parents,
            shown_parents,
            "{commit}'s parents from the details query"
        );
    }
    assert_eq!(
        clone
            .git(&["log", "-1", "--format=%P", "HEAD"])
            .split_whitespace()
            .count(),
        1,
        "the commit above the boundary has its parent, so the fixture shows both cases"
    );
    let boundary = hex(&clone, "HEAD~1");
    let found = changes_of(
        &clone,
        &ChangesRequest::commit(Oid::parse(&boundary).expect("an id")),
    );
    assert!(
        found
            .files
            .iter()
            .all(|file| matches!(file.status, ChangeStatus::Added)),
        "the boundary is shown as a root: {:?}",
        found.files
    );
}

/// Git `2.<minor>.0`, to compare the git in use against.
fn since(minor: u32) -> cairn_git::ops::GitVersion {
    cairn_git::ops::GitVersion {
        major: 2,
        minor,
        patch: 0,
    }
}

/// `git log --raw` for one commit in the repository at `git_dir`, named explicitly, as
/// the oracle for a repository git's discovery would not find.
fn shown_in(holder: &Repo, git_dir: &str, commit: &str, env: &[(&str, &str)]) -> Vec<Row> {
    let (status, stdout, stderr) = holder.run(
        &[
            "--git-dir",
            git_dir,
            "log",
            "-1",
            "--raw",
            "--no-abbrev",
            "--format=",
            commit,
        ],
        env,
        None,
    );
    assert!(status.success(), "git log failed: {stderr}");
    rows_of(&stdout)
}

/// A repository whose git directory lives apart from its working tree (`core.worktree`),
/// with that working tree inside another repository's: run from the working tree, git's
/// discovery finds the ENCLOSING repository and answers "bad object" for every commit of
/// this one. The query names the repository it opened, so it answers what that
/// repository's own `git log` shows. Caught by: an invocation left to discovery.
#[test]
fn a_repository_whose_working_tree_sits_inside_another_is_the_one_asked() {
    let enclosing = repositories::crafted();
    let inner = enclosing.path().join("inner");
    ok(std::fs::create_dir_all(&inner), "the inner working tree");
    let holder = Repo::new("separate-holder");
    let git_dir = holder.path().join("separate.git");
    let git_dir_text = git_dir.display().to_string();
    let inner_text = inner.display().to_string();
    let in_separate = |args: &[&str]| {
        let mut all = vec![
            "--git-dir",
            git_dir_text.as_str(),
            "--work-tree",
            &inner_text,
        ];
        all.extend_from_slice(args);
        holder.git(&all)
    };
    holder.git(&["init", "--quiet", "--bare", &git_dir_text]);
    holder.git(&["--git-dir", &git_dir_text, "config", "core.bare", "false"]);
    holder.git(&[
        "--git-dir",
        &git_dir_text,
        "config",
        "core.worktree",
        &inner_text,
    ]);
    ok(std::fs::write(inner.join("a.txt"), "one\n"), "a file");
    in_separate(&["add", "a.txt"]);
    in_separate(&["commit", "--quiet", "-m", "first"]);
    ok(std::fs::write(inner.join("a.txt"), "two\n"), "a file");
    ok(std::fs::write(inner.join("b.txt"), "new\n"), "a file");
    in_separate(&["add", "a.txt", "b.txt"]);
    in_separate(&["commit", "--quiet", "-m", "second"]);
    let head = in_separate(&["rev-parse", "HEAD"]).trim().to_owned();
    let inner_repo = Repo::borrowed(&inner);
    assert!(
        inner_repo
            .try_git(&["cat-file", "-e", &head], &[], None)
            .is_err(),
        "git's discovery from the working tree must find the enclosing repository, or \
         this test proves nothing"
    );

    let found = changes_of(
        &Repo::borrowed(&git_dir),
        &ChangesRequest::commit(Oid::parse(&head).expect("an id")),
    );
    let expected = shown_in(&holder, &git_dir_text, &head, &[]);
    assert_eq!(expected.len(), 2, "{expected:?}");
    assert_eq!(cairn_rows(&found.files, head.len()), expected);
}

/// Under `safe.bareRepository=explicit` git refuses to discover a bare repository, so a
/// query run inside one by discovery exits 128 on every commit. The query names the
/// repository it opened, which is the explicit spelling the setting asks for, and
/// answers what `git --git-dir=<it> log` shows. Caught by: an invocation left to
/// discovery. The setting arrived in git 2.38; an older git has nothing to refuse, and
/// there this says so and decides nothing.
#[test]
fn a_bare_repository_is_answered_under_safe_bare_repository_explicit() {
    if git().version() < since(38) {
        eprintln!(
            "SKIPPED a_bare_repository_is_answered_under_safe_bare_repository_explicit: git {} \
             has no safe.bareRepository",
            git().version()
        );
        return;
    }
    let source = repositories::crafted();
    let holder = Repo::new("bare-holder");
    let bare = holder.path().join("bare.git");
    let bare_text = bare.display().to_string();
    holder.git(&[
        "clone",
        "--quiet",
        "--bare",
        &source.path().display().to_string(),
        &bare_text,
    ]);
    let home = holder.path().join("home");
    ok(std::fs::create_dir_all(&home), "a home");
    let global = home.join(".gitconfig");
    ok(
        std::fs::write(&global, "[safe]\n\tbareRepository = explicit\n"),
        "a global configuration",
    );
    let global_text = global.display().to_string();
    let explicit = [("GIT_CONFIG_GLOBAL", global_text.as_str())];
    assert!(
        Repo::borrowed(&bare)
            .try_git(&["log", "-1"], &explicit, None)
            .is_err(),
        "git discovers the bare repository anyway, so this test proves nothing"
    );

    let home_text = home.clone().into_os_string();
    let binary = ok(
        cairn_git::ops::GitBinary::discover_with(cairn_git::ops::GitEnvironment::new(
            |name| match name {
                "PATH" => std::env::var_os("PATH"),
                "HOME" => Some(home_text.clone()),
                _ => None,
            },
            &cairn_git::ops::Askpass::new("/nonexistent/cairn-askpass", None),
        )),
        "git is found",
    );
    let engine = ok(Repository::discover(&bare), "the bare repository opens");
    assert!(engine.workdir().is_none(), "the fixture is bare");
    let head = hex(&Repo::borrowed(&bare), "HEAD");
    let found = ok(
        engine.changes(
            &binary,
            &ChangesRequest::commit(Oid::parse(&head).expect("an id")),
            &CancelSignal::new(),
        ),
        "the query answers in a bare repository",
    );
    let expected = shown_in(&holder, &bare_text, &head, &explicit);
    assert!(!expected.is_empty(), "the head commit changes something");
    assert_eq!(cairn_rows(&found.files, head.len()), expected);
}

/// The rows of `git show --raw` for one commit: the other porcelain a user reads a commit
/// through, which reads `log.showRoot` as `git log` does.
fn shown_by_show(repo: &Repo, commit: &str) -> Vec<Row> {
    rows_of(&repo.git(&["show", "--raw", "--no-abbrev", "--format=", commit]))
}

/// `log.showRoot`, which the user's own `git log` and `git show` read and plumbing does
/// not: unset, true or the bare key, a root commit's diff is its whole content; false, it
/// is no diff at all — for a shallow clone's boundary commit too, which git shows as a
/// root. A commit with a parent, and a comparison of two commits, are the same under every
/// value; and a comparison does not refuse a value `git log` refuses, since `git diff`
/// never reads it. Caught by: the key ignored (a root always diffed), read as git's
/// opposite default, the bare key read as false, or applied to a comparison.
#[test]
fn the_root_commit_is_shown_as_log_show_root_says() {
    let values: &[Option<&str>] = &[
        None,
        Some("true"),
        Some("false"),
        Some("0"),
        Some("no"),
        Some("BARE"),
    ];
    let (mut whole, mut nothing) = (0, 0);
    for value in values {
        let repo = repositories::crafted();
        match value {
            None => {}
            Some("BARE") => append_config(&repo, "[log]\n\tshowRoot\n"),
            Some(value) => repo.config("log.showRoot", value),
        }
        let root = repo
            .git(&["rev-list", "--max-parents=0", "HEAD"])
            .trim()
            .to_owned();
        let head = hex(&repo, "HEAD");
        for commit in [&root, &head] {
            let found = changes_of(
                &repo,
                &ChangesRequest::commit(Oid::parse(commit).expect("an id")),
            );
            let (rows, _) = shown(&repo, commit);
            assert_eq!(
                cairn_rows(&found.files, commit.len()),
                rows,
                "log.showRoot={value:?} on {commit}"
            );
            assert_eq!(rows, shown_by_show(&repo, commit), "git show agrees");
            assert!(found.details.is_some(), "the details are there either way");
            if commit == &root {
                if rows.is_empty() {
                    nothing += 1;
                } else {
                    whole += 1;
                }
            } else {
                assert!(!rows.is_empty(), "a commit with a parent is always diffed");
            }
        }
        let between = changes_of(
            &repo,
            &ChangesRequest::between(
                Oid::parse(&root).expect("an id"),
                Oid::parse(&head).expect("an id"),
            ),
        );
        assert_eq!(
            cairn_rows(&between.files, head.len()),
            rows_of(&repo.git(&["diff", "--raw", "--no-abbrev", &root, &head])),
            "a comparison is git diff's, whatever log.showRoot={value:?} says"
        );
    }
    assert_eq!((whole, nothing), (3, 3), "each value decided something");

    let repo = repositories::crafted();
    repo.config("log.showRoot", "bogus");
    let root =
        Oid::parse(repo.git(&["rev-list", "--max-parents=0", "HEAD"]).trim()).expect("an id");
    let head = Oid::parse(&hex(&repo, "HEAD")).expect("an id");
    assert!(
        try_changes(&repo, &ChangesRequest::between(root, head)).is_ok(),
        "git diff does not read log.showRoot, so a comparison does not refuse it"
    );

    let source = repositories::crafted();
    let holder = Repo::new("shallow-root-holder");
    let clone_path = holder.path().join("clone");
    holder.git(&[
        "clone",
        "-q",
        "--depth",
        "1",
        &format!("file://{}", source.path().display()),
        &clone_path.to_string_lossy(),
    ]);
    let clone = Repo::borrowed(&clone_path);
    clone.config("log.showRoot", "false");
    let boundary = hex(&clone, "HEAD");
    let found = changes_of(
        &clone,
        &ChangesRequest::commit(Oid::parse(&boundary).expect("an id")),
    );
    let (rows, _) = shown(&clone, &boundary);
    assert!(
        rows.is_empty(),
        "git log shows the boundary as a root: {rows:?}"
    );
    assert_eq!(cairn_rows(&found.files, boundary.len()), rows);
}

/// `diff.ignoreSubmodules`, which the user's own `git log` and `git diff` read and plumbing
/// does not, against `git log --raw` (and, for a comparison, `git diff --raw`) under each
/// way a user sets it — globally, per submodule in the configuration (keyed by the name
/// `.gitmodules` gives, never by the path), per submodule in the working tree's
/// `.gitmodules`, and an override back to `all` — with `diff.renameLimit` at 1, so that an
/// added gitlink git log never counted would cut the rename search short if the query
/// counted it. Caught by: the key ignored (every gitlink listed); `--ignore-submodules=all`
/// passed whatever a submodule's own setting says (the `none` cases); a list filtered
/// after git answered (the rename shown unpaired, and the cut-short reported, where git log
/// pairs it); a per-submodule setting looked up by path rather than by name; or the global
/// applied when it is not `all`.
#[test]
fn a_submodule_is_listed_as_diff_ignore_submodules_shows_it() {
    struct Case {
        config: &'static [(&'static str, &'static str)],
        gitmodules: Option<&'static str>,
        shown: &'static [&'static str],
    }
    let cases = [
        Case {
            config: &[],
            gitmodules: None,
            shown: &["s", "t", "u"],
        },
        Case {
            config: &[("diff.ignoreSubmodules", "dirty")],
            gitmodules: None,
            shown: &["s", "t", "u"],
        },
        Case {
            config: &[("diff.ignoreSubmodules", "all")],
            gitmodules: None,
            shown: &[],
        },
        Case {
            config: &[
                ("diff.ignoreSubmodules", "all"),
                ("submodule.s.ignore", "none"),
            ],
            gitmodules: None,
            shown: &["s"],
        },
        Case {
            config: &[
                ("diff.ignoreSubmodules", "all"),
                ("submodule.t.ignore", "none"),
            ],
            gitmodules: None,
            shown: &[],
        },
        Case {
            config: &[
                ("diff.ignoreSubmodules", "all"),
                ("submodule.named.ignore", "untracked"),
            ],
            gitmodules: None,
            shown: &["t"],
        },
        Case {
            config: &[("diff.ignoreSubmodules", "all")],
            gitmodules: Some(
                "[submodule \"s\"]\n\tpath = s\n\tignore = dirty\n\
                 [submodule \"named\"]\n\tpath = t\n",
            ),
            shown: &["s"],
        },
        Case {
            config: &[
                ("diff.ignoreSubmodules", "all"),
                ("submodule.s.ignore", "all"),
            ],
            gitmodules: Some("[submodule \"s\"]\n\tpath = s\n\tignore = none\n"),
            shown: &[],
        },
        Case {
            config: &[
                ("diff.ignoreSubmodules", "none"),
                ("submodule.s.ignore", "all"),
            ],
            gitmodules: None,
            shown: &["t", "u"],
        },
    ];
    let mut paired = 0;
    for case in &cases {
        let mut config = vec![("diff.renameLimit", "1")];
        config.extend_from_slice(case.config);
        let repo = repositories::submodules(&config);
        if let Some(text) = case.gitmodules {
            // The working tree's file, unstaged: what git reads, and not the commit's.
            repo.write(".gitmodules", text.as_bytes());
        }
        let label = format!("{:?} with .gitmodules {:?}", case.config, case.gitmodules);
        let head = hex(&repo, "HEAD");
        let found = changes_of(
            &repo,
            &ChangesRequest::commit(Oid::parse(&head).expect("an id")),
        );
        let (rows, stderr) = shown(&repo, &head);
        assert_eq!(cairn_rows(&found.files, head.len()), rows, "{label}");
        assert_eq!(
            found.renames.needed_limit,
            warned_limit(&stderr),
            "{label}: git said {stderr:?}"
        );
        let mut gitlinks: Vec<&str> = rows
            .iter()
            .filter(|row| row.new_mode == "160000")
            .map(|row| row.new_path.as_str())
            .collect();
        gitlinks.sort_unstable();
        assert_eq!(gitlinks, case.shown, "{label}: the fixture decides nothing");
        paired += usize::from(rows.iter().any(|row| row.status.starts_with('R')));

        let old = hex(&repo, "HEAD~1");
        let between = changes_of(
            &repo,
            &ChangesRequest::between(
                Oid::parse(&old).expect("an id"),
                Oid::parse(&head).expect("an id"),
            ),
        );
        assert_eq!(
            cairn_rows(&between.files, head.len()),
            rows_of(&repo.git(&["diff", "--raw", "--no-abbrev", &old, &head])),
            "{label}: a comparison is git diff's"
        );
    }
    assert!(
        (1..cases.len()).contains(&paired),
        "the rename was paired in {paired} cases, so the limit never decided anything"
    );
}

/// A submodule whose own setting git refuses makes the user's `git log` refuse the
/// commit, and the query too — `diff-tree` reads that setting itself, so it is asked
/// rather than told to hide every gitlink. Caught by: `--ignore-submodules=all` passed
/// whenever the global is `all`, which never reads the setting and answers.
#[test]
fn a_submodule_setting_git_refuses_is_refused() {
    let repo = repositories::submodules(&[]);
    repo.config("diff.ignoreSubmodules", "all");
    repo.config("submodule.s.ignore", "bogus");
    let head = hex(&repo, "HEAD");
    let (status, _, _) = repo.run(&["log", "-1", "--raw", "--format=", &head], &[], None);
    assert!(
        !status.success(),
        "git log answers, so this decides nothing"
    );
    let refused = try_changes(
        &repo,
        &ChangesRequest::commit(Oid::parse(&head).expect("an id")),
    );
    assert!(
        matches!(refused, Err(Error::GitFailed { .. })),
        "{refused:?}"
    );
}

/// A bare repository has no working tree, so git reads no `.gitmodules` at all, and no
/// submodule has a setting of its own: under `diff.ignoreSubmodules=all` every gitlink is
/// hidden, whatever `HEAD`'s `.gitmodules` says. Caught by: reading `.gitmodules` from the
/// tree where git has no working tree to read it from.
#[test]
fn a_bare_repository_reads_no_gitmodules() {
    let source = repositories::submodules(&[]);
    source.write(
        ".gitmodules",
        b"[submodule \"s\"]\n\tpath = s\n\tignore = none\n",
    );
    source.git(&["add", ".gitmodules"]);
    source.git(&["commit", "--quiet", "-m", "show s"]);
    let holder = Repo::new("bare-submodules");
    let bare = holder.path().join("bare.git");
    holder.git(&[
        "clone",
        "--quiet",
        "--bare",
        &source.path().display().to_string(),
        &bare.display().to_string(),
    ]);
    let repo = Repo::borrowed(&bare);
    repo.config("diff.ignoreSubmodules", "all");
    let commit = hex(&repo, "HEAD~1");
    let found = changes_of(
        &repo,
        &ChangesRequest::commit(Oid::parse(&commit).expect("an id")),
    );
    let (rows, _) = shown(&repo, &commit);
    assert!(
        rows.iter().all(|row| row.new_mode != "160000"),
        "git log in a bare repository hides every gitlink: {rows:?}"
    );
    assert_eq!(cairn_rows(&found.files, commit.len()), rows);
    // The same commit in the source, which has a working tree and so reads its file.
    let (with_tree, _) = shown(&source, &commit);
    assert!(
        with_tree.iter().any(|row| row.new_path == "s"),
        "the source shows s, so the bare case decides something: {with_tree:?}"
    );
}
