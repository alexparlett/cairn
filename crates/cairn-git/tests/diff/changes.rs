//! C5: the changes query against git's own detection, under the same configuration.

use cairn_git::{CancelSignal, Error};
use cairn_git::{ChangeSet, ChangesRequest, Repository};
use cairn_model::{ChangeStatus, ChangedFile, FileMode, Oid};

use super::repositories::{self, Repo};
use super::{ok, some};

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

/// `git diff-tree -r --raw <flags> <revs>`, parsed. Lines that do not start with `:` are
/// the commit-id header git prints when it is given one commit.
fn git_rows(repo: &Repo, flags: &[&str], revs: &[&str]) -> Vec<Row> {
    let mut args = vec!["diff-tree", "-r", "--raw", "--no-abbrev", "--no-ext-diff"];
    args.extend_from_slice(flags);
    args.extend_from_slice(revs);
    let out = repo.git(&args);
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

fn changes_of(repo: &Repo, request: &ChangesRequest) -> ChangeSet {
    let engine = ok(Repository::discover(repo.path()), "the fixture opens");
    ok(
        engine.changes(request, &CancelSignal::new()),
        "the changes query answers",
    )
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
    assert!(
        !found.renames.enabled,
        "detection was off: {:?}",
        found.renames
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

/// R2.2's second half: when `diff.renameLimit` stops the search, the answer says so.
#[test]
fn a_rename_limit_that_cuts_detection_short_is_reported() {
    let limited = repositories::rewrites(&[("diff.renames", "true"), ("diff.renameLimit", "1")]);
    let rename = hex(&limited, "HEAD~1");
    let found = changes_of(
        &limited,
        &ChangesRequest::commit(Oid::parse(&rename).expect("an id")),
    );
    assert!(
        found.renames.was_cut_short(),
        "a limit of one cannot cover four deletions against four additions: {:?}",
        found.renames
    );
    assert_eq!(found.renames.limit, 1);

    let ample = repositories::rewrites(&[("diff.renames", "true"), ("diff.renameLimit", "1000")]);
    let rename = hex(&ample, "HEAD~1");
    let found = changes_of(
        &ample,
        &ChangesRequest::commit(Oid::parse(&rename).expect("an id")),
    );
    assert!(
        !found.renames.was_cut_short(),
        "the same commit under git's own limit is not cut short: {:?}",
        found.renames
    );
    assert_eq!(found.renames.limit, 1000);
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

/// R2.9: the walk stops rather than finishing and throwing the answer away.
#[test]
fn a_cancelled_changes_query_stops_walking() {
    struct StopsAfter(std::cell::Cell<usize>);
    impl cairn_git::Cancel for StopsAfter {
        fn is_cancelled(&self) -> bool {
            let seen = self.0.get();
            self.0.set(seen + 1);
            seen >= 2
        }
    }

    let repo = repositories::rewrites(&[("diff.renames", "false")]);
    let engine = Repository::discover(repo.path()).expect("the fixture opens");
    let seed = Oid::parse(&hex(&repo, "HEAD~2")).expect("an id");
    let request = ChangesRequest::commit(seed);

    let whole = engine
        .changes(&request, &CancelSignal::new())
        .expect("uncancelled");
    assert!(
        whole.files.len() > 3,
        "the fixture has files to stop short of"
    );

    let counter = StopsAfter(std::cell::Cell::new(0));
    let error = engine.changes(&request, &counter).unwrap_err();
    let Error::ChangesCancelled { changed } = error else {
        panic!("a cancelled query must say so: {error:?}");
    };
    assert!(
        changed < whole.files.len(),
        "the walk ran to the end anyway: {changed} of {}",
        whole.files.len()
    );
    assert!(
        counter.0.get() <= whole.files.len(),
        "the flag was polled after the break"
    );
}

/// R2.10, and the failure a view has to draw something for.
#[test]
fn a_commit_that_is_not_there_is_refused() {
    let repo = repositories::crafted();
    let engine = Repository::discover(repo.path()).expect("the fixture opens");
    let missing = Oid::parse("1234567890abcdef1234567890abcdef12345678").expect("an id");
    let error = engine
        .changes(&ChangesRequest::commit(missing), &CancelSignal::new())
        .unwrap_err();
    assert!(matches!(error, Error::ReadCommit { .. }), "got {error:?}");
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
