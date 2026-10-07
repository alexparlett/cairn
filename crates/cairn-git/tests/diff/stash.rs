//! C7's engine half (refs-and-status R6.2): a stash lists what `git stash show --name-status`
//! lists, with `stash.showIncludeUntracked` unset and set — the untracked files paired with
//! the tracked changes as git pairs them — and each file's content is read from the side it
//! is on, the untracked commit's included. Run under the host's git and the floors' (git
//! 2.30.9 ignores the setting, as it ignores it for the user).

use std::collections::BTreeSet;

use cairn_git::{CancelSignal, ChangesRequest, ContentOptions, Error, Repository};
use cairn_model::{ChangeStatus, ChangedFile, DiffContent, Oid};

use super::repositories::Repo;
use super::{every_file, git, ok, since};

/// A stash made with `--include-untracked` of: a tracked file deleted and its content, one
/// line edited, left untracked under another name (git pairs the two, and the edit makes
/// the content query ask git which line changed), a tracked file edited, and a new
/// untracked file. Returns the repository and the stash commit.
fn stashed() -> (Repo, Oid) {
    let repo = Repo::new("stash");
    let body: String = (0..60).map(|n| format!("line {n} of a\n")).collect();
    repo.write("a", body.as_bytes());
    repo.write("kept.txt", b"one\ntwo\nthree\n");
    repo.commit("base");
    repo.remove("a");
    repo.write(
        "b",
        body.replace("line 30 of a", "line 30 moved").as_bytes(),
    );
    repo.write("kept.txt", b"one\nTWO\nthree\n");
    repo.write("new.txt", b"brand new\n");
    repo.git(&[
        "stash",
        "push",
        "--quiet",
        "--include-untracked",
        "-m",
        "wip",
    ]);
    let stash = repo.rev("stash@{0}");
    (repo, stash)
}

/// `(status letter, paths)` for each file, as `--name-status` spells it with the score
/// left off.
fn listed(files: &[ChangedFile]) -> BTreeSet<(String, Vec<String>)> {
    files
        .iter()
        .map(|file| {
            let letter = match file.status {
                ChangeStatus::Added => "A",
                ChangeStatus::Deleted => "D",
                ChangeStatus::Modified => "M",
                ChangeStatus::TypeChanged => "T",
                ChangeStatus::Renamed(_) => "R",
                ChangeStatus::Copied(_) => "C",
            };
            let paths = match file.status {
                ChangeStatus::Renamed(_) | ChangeStatus::Copied(_) => {
                    vec![file.old_path.to_string(), file.new_path.to_string()]
                }
                ChangeStatus::Added
                | ChangeStatus::Deleted
                | ChangeStatus::Modified
                | ChangeStatus::TypeChanged => vec![file.new_path.to_string()],
            };
            (letter.to_owned(), paths)
        })
        .collect()
}

/// What the user's own `git stash show --name-status` prints, read the same way.
fn git_lists(repo: &Repo) -> BTreeSet<(String, Vec<String>)> {
    let printed = repo.git(&["stash", "show", "--name-status", "-z", "stash@{0}"]);
    let mut fields = printed.split('\0').filter(|field| !field.is_empty());
    let mut listed = BTreeSet::new();
    while let Some(status) = fields.next() {
        let letter = status.get(..1).unwrap_or_default().to_owned();
        let count = if letter == "R" || letter == "C" { 2 } else { 1 };
        let paths = (0..count)
            .filter_map(|_| fields.next())
            .map(str::to_owned)
            .collect();
        listed.insert((letter, paths));
    }
    listed
}

fn stash_changes(repo: &Repo, stash: Oid) -> Result<cairn_model::ChangeSet, Error> {
    let engine = ok(Repository::discover(repo.path()), "the fixture opens");
    engine.changes(git(), &ChangesRequest::stash(stash), &CancelSignal::new())
}

/// C7, R6.2: with `stash.showIncludeUntracked` unset, a stash lists its tracked changes
/// alone; set, the untracked files too, a deleted tracked file and an untracked file of its
/// content paired as the rename git calls them — each exactly what `git stash show
/// --name-status` lists under the same git. Caught by: a commit's comparison of the stash
/// (no untracked files ever), untracked files listed with the setting unset, or the two
/// halves diffed apart (`D a` and `A b`, where git pairs them).
#[test]
fn a_stash_lists_what_git_stash_show_lists_with_the_setting_unset_and_set() {
    let (repo, stash) = stashed();
    let pairs = git().version() >= since(32);

    repo.config("stash.showIncludeUntracked", "false");
    let unset = listed(&ok(stash_changes(&repo, stash), "the stash answers").files);
    assert_eq!(
        unset,
        git_lists(&repo),
        "unset: not what git stash show lists"
    );
    assert!(
        !unset
            .iter()
            .any(|(_, paths)| paths.contains(&"new.txt".to_owned())),
        "an untracked file was listed with the setting unset: {unset:?}"
    );

    repo.config("stash.showIncludeUntracked", "true");
    let set = listed(&ok(stash_changes(&repo, stash), "the stash answers").files);
    assert_eq!(set, git_lists(&repo), "set: not what git stash show lists");
    if pairs {
        assert!(
            set.contains(&("R".to_owned(), vec!["a".to_owned(), "b".to_owned()])),
            "the tracked deletion and the untracked file were not paired: {set:?}"
        );
        assert!(
            set.contains(&("A".to_owned(), vec!["new.txt".to_owned()])),
            "{set:?}"
        );
    } else {
        assert_eq!(set, unset, "a git before 2.32 does not read the setting");
        eprintln!(
            "note: git {:?} predates stash.showIncludeUntracked; only the unset answer was decided",
            git().version()
        );
    }
}

/// R6.2: each file of a stash reads its content from the side it is on — a tracked edit from
/// the stash commit, an untracked file and a rename into one from the untracked commit —
/// alone and through Expand All's pages, each side the bytes git stores. Caught by: every
/// file read between the base and the stash commit, which holds no untracked file (the read
/// fails), or a pair asked as git does not list it between the trees it is read from.
#[test]
fn each_file_of_a_stash_is_read_from_the_side_it_is_on() {
    let (repo, stash) = stashed();
    repo.config("stash.showIncludeUntracked", "true");
    let request = ChangesRequest::stash(stash);
    let engine = ok(Repository::discover(repo.path()), "the fixture opens");
    let set = ok(
        engine.changes(git(), &request, &CancelSignal::new()),
        "the stash answers",
    );
    let mut session = ok(engine.diff_session(), "a diff session");
    let options = ContentOptions::default();
    for file in &set.files {
        let diff = ok(
            session.file_diff(git(), &request, file, &options, &CancelSignal::new()),
            &format!("{} reads", file.new_path),
        );
        let DiffContent::Text { text, .. } = &diff.content else {
            panic!("{} is not text: {:?}", file.new_path, diff.content);
        };
        for (id, content) in [
            (file.old_id, text.old_content()),
            (file.new_id, text.new_content()),
        ] {
            match id {
                Some(id) => assert_eq!(
                    String::from_utf8_lossy(&content),
                    repo.git(&["cat-file", "blob", id.hex().as_str()]),
                    "{} does not hold the bytes of {id}",
                    file.new_path
                ),
                None => assert!(content.is_empty()),
            }
        }
        let changed = text.changes();
        assert!(!changed.is_empty(), "{} shows no change", file.new_path);
        if matches!(file.status, ChangeStatus::Renamed(_)) {
            assert_eq!(
                changed.len(),
                1,
                "the rename's one edited line: {changed:?}"
            );
        }
    }
    let paged = ok(
        every_file(&mut session, &request, &set, &options, &CancelSignal::new()),
        "Expand All reads every file",
    );
    assert_eq!(paged.len(), set.files.len());
    if git().version() >= since(32) {
        assert!(
            set.files
                .iter()
                .any(|file| file.new_path.to_string() == "new.txt"),
            "no untracked file was listed, so none was read from its commit"
        );
    }
}

fn snapshot(dir: &std::path::Path) -> std::collections::BTreeMap<std::path::PathBuf, Vec<u8>> {
    let mut files = std::collections::BTreeMap::new();
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

/// The stash read writes nothing: the git directory is byte-identical after a stash's list
/// and every file's content, with the setting set and the working tree stat-dirty, so an
/// index refresh would have something to write. Caught by: a read that refreshed or wrote
/// the index, a ref or an object.
#[test]
fn a_stash_read_writes_nothing() {
    let (repo, stash) = stashed();
    repo.config("stash.showIncludeUntracked", "true");
    std::thread::sleep(std::time::Duration::from_millis(20));
    repo.write("kept.txt", b"one\ntwo\nthree\n");
    let before = snapshot(&repo.path().join(".git"));

    let request = ChangesRequest::stash(stash);
    let engine = ok(Repository::discover(repo.path()), "the fixture opens");
    let set = ok(
        engine.changes(git(), &request, &CancelSignal::new()),
        "the stash answers",
    );
    assert!(
        !set.files.is_empty(),
        "nothing was read, so nothing decided"
    );
    let mut session = ok(engine.diff_session(), "a diff session");
    let _ = every_file(
        &mut session,
        &request,
        &set,
        &ContentOptions::default(),
        &CancelSignal::new(),
    );

    assert_eq!(
        snapshot(&repo.path().join(".git")),
        before,
        "the git directory changed under a stash read"
    );
}

/// R6.2's Commit tab: a stash's change set carries the stash commit's own details — its
/// message, and every parent, the untracked commit among them.
#[test]
fn a_stash_describes_its_stash_commit() {
    let (repo, stash) = stashed();
    let set = ok(stash_changes(&repo, stash), "the stash answers");
    let details = set.details.unwrap_or_else(|| panic!("no details"));
    assert_eq!(details.id, stash);
    assert_eq!(
        details.parents.len(),
        3,
        "base, index and untracked commits"
    );
    assert!(details.message.contains("wip"), "{:?}", details.message);
}

/// Under `diff.renames=copies`, an untracked file copied from a tracked file the stash also
/// edits is the copy git lists — and its content is read between the base and the untracked
/// commit, where that source is absent, so it is asked as the rename it is there: the same
/// two blobs. Caught by: the copy asked as a copy between those trees (git answers a rename,
/// and the read fails), or read from the stash commit, which does not hold it.
#[test]
fn a_copy_into_an_untracked_file_is_read_from_the_untracked_commit() {
    if git().version() < since(32) {
        eprintln!("SKIPPED a_copy_into_an_untracked_file: git before 2.32 lists no untracked file");
        return;
    }
    let repo = Repo::new("stash-copy");
    let body: String = (0..60)
        .map(|n| format!("line {n} of the source\n"))
        .collect();
    repo.write("source.txt", body.as_bytes());
    repo.commit("base");
    repo.write("source.txt", format!("{body}edited\n").as_bytes());
    let copied = body.replace("line 7 of the source", "line 7 of the copy");
    repo.write("copy.txt", copied.as_bytes());
    repo.git(&["stash", "push", "--quiet", "--include-untracked"]);
    let stash = repo.rev("stash@{0}");
    repo.config("diff.renames", "copies");
    repo.config("stash.showIncludeUntracked", "true");

    let request = ChangesRequest::stash(stash);
    let engine = ok(Repository::discover(repo.path()), "the fixture opens");
    let set = ok(
        engine.changes(git(), &request, &CancelSignal::new()),
        "the stash answers",
    );
    assert_eq!(listed(&set.files), git_lists(&repo));
    let copy = set
        .files
        .iter()
        .find(|file| matches!(file.status, ChangeStatus::Copied(_)))
        .unwrap_or_else(|| panic!("git listed no copy: {:?}", set.files));
    let mut session = ok(engine.diff_session(), "a diff session");
    let diff = ok(
        session.file_diff(
            git(),
            &request,
            copy,
            &ContentOptions::default(),
            &CancelSignal::new(),
        ),
        "the copy reads",
    );
    assert_eq!(&diff.file, copy, "the answer is not the file asked about");
    let DiffContent::Text { text, .. } = &diff.content else {
        panic!("the copy is not text: {:?}", diff.content);
    };
    assert_eq!(text.changes().len(), 1, "the copy's one edited line");
    assert_eq!(text.new_content(), copied.as_bytes());
}
