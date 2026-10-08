//! staging-and-commit's C7 (R2.6, L18): the staged diff of a path pairs a rename or a copy
//! exactly as the user's `git diff --cached` pairs it — under `diff.renames` unset, `false`,
//! `copies`, and a `diff.renameLimit` that cuts the search short — and draws the pair's
//! lines as git draws them.
//!
//! The oracle is porcelain `git diff --cached --raw -z` over the whole index, run with the
//! fixture's own configuration: what it pairs a path into is the record the engine must
//! answer with, field for field, and a path it lists in no record is a path the engine
//! answers nothing for. A pathspec of one path would be no oracle at all — `git diff --cached
//! -- <new>` shows a staged rename as an addition (`git_diff_cached_of_one_path_pairs_nothing`).

use cairn_git::WorkingTreeDiff;
use cairn_model::{ChangeStatus, ChangedFile, FileMode, Oid, RepoPath, Similarity};

use super::repositories::Repo;
use super::working_tree::{ask, same_as_git};

/// Twenty lines, distinct per file, so similarity is what the edits make it.
fn lines(stem: &str) -> String {
    (0..20).map(|n| format!("{stem} {n:02}\n")).collect()
}

/// A repository with every kind of staged change around a rename and a copy: `old.txt`
/// renamed to `new.txt` with an edit, `copy.txt` a copy of `src.txt` with `src.txt` itself
/// edited, a deleted empty file beside an intent-to-add one (which plumbing would pair and
/// porcelain does not see), a plain edit and an unrelated addition.
fn staged_rewrites(config: &[(&str, &str)]) -> Repo {
    let repo = Repo::new("c7");
    for (key, value) in config {
        repo.config(key, value);
    }
    repo.write("old.txt", lines("old").as_bytes());
    repo.write("src.txt", lines("source").as_bytes());
    repo.write("plain.txt", lines("plain").as_bytes());
    repo.write("empty", b"");
    repo.commit("base");

    repo.git(&["mv", "old.txt", "new.txt"]);
    repo.write(
        "new.txt",
        lines("old").replace("old 05", "renamed 05").as_bytes(),
    );
    repo.write("copy.txt", lines("source").as_bytes());
    repo.write(
        "src.txt",
        format!("{}one more\n", lines("source")).as_bytes(),
    );
    repo.write(
        "plain.txt",
        lines("plain").replace("plain 10", "PLAIN").as_bytes(),
    );
    repo.write("added.txt", b"nothing like the others\n");
    repo.git(&["rm", "--quiet", "--cached", "empty"]);
    std::fs::remove_file(repo.path().join("empty")).unwrap_or_else(|e| panic!("{e}"));
    repo.git(&[
        "add",
        "new.txt",
        "copy.txt",
        "src.txt",
        "plain.txt",
        "added.txt",
    ]);
    repo.write("ita", b"");
    repo.git(&["add", "-N", "ita"]);
    repo
}

const PATHS: [&str; 8] = [
    "new.txt",
    "old.txt",
    "copy.txt",
    "src.txt",
    "plain.txt",
    "added.txt",
    "empty",
    "ita",
];

/// Every record of porcelain `git diff --cached --raw -z --no-abbrev` over the whole index.
fn porcelain_records(repo: &Repo) -> Vec<ChangedFile> {
    let raw = repo.git(&["diff", "--cached", "--raw", "-z", "--no-abbrev"]);
    let mut fields = raw.split('\0').filter(|field| !field.is_empty());
    let mut records = Vec::new();
    while let Some(meta) = fields.next() {
        let meta = meta.strip_prefix(':').unwrap_or_else(|| panic!("{meta:?}"));
        let parts: Vec<&str> = meta.split(' ').collect();
        let [old_mode, new_mode, old_id, new_id, status] = parts[..] else {
            panic!("a record git did not print: {meta:?}");
        };
        let (letter, score) = status.split_at(1);
        let similarity = || Similarity::from_percent(score.parse().unwrap_or(0));
        let status = match letter {
            "A" => ChangeStatus::Added,
            "D" => ChangeStatus::Deleted,
            "M" => ChangeStatus::Modified,
            "T" => ChangeStatus::TypeChanged,
            "R" => ChangeStatus::Renamed(similarity()),
            "C" => ChangeStatus::Copied(similarity()),
            other => panic!("status {other}"),
        };
        let first = fields.next().unwrap_or_else(|| panic!("a path"));
        let second = match status {
            ChangeStatus::Renamed(_) | ChangeStatus::Copied(_) => {
                fields.next().unwrap_or_else(|| panic!("a second path"))
            }
            _ => first,
        };
        let mode = |digits: &str| FileMode::from_octal(digits);
        let id = |hex: &str| {
            Oid::parse(hex)
                .ok()
                .filter(|id| !id.as_bytes().iter().all(|b| *b == 0))
        };
        records.push(ChangedFile {
            status,
            old_path: RepoPath::from(first),
            new_path: RepoPath::from(second),
            old_mode: mode(old_mode),
            new_mode: mode(new_mode),
            old_id: id(old_id),
            new_id: id(new_id),
        });
    }
    records
}

/// The record `git diff --cached` shows `path` in: as a destination, or as a rename's
/// source — never as a copy's, which keeps a record of its own.
fn record_for<'a>(records: &'a [ChangedFile], path: &str) -> Option<&'a ChangedFile> {
    let path = RepoPath::from(path);
    records
        .iter()
        .find(|file| file.new_path == path)
        .or_else(|| {
            records
                .iter()
                .find(|file| file.is_rename() && file.old_path == path)
        })
}

/// C7 itself, for each configuration: every path's staged record is git's, and its lines
/// are git's lines for it.
#[test]
fn the_staged_diff_pairs_renames_and_copies_as_git_diff_cached_does() {
    let configurations: [(&str, &[(&str, &str)]); 5] = [
        ("diff.renames unset", &[]),
        ("diff.renames=false", &[("diff.renames", "false")]),
        ("diff.renames=true", &[("diff.renames", "true")]),
        ("diff.renames=copies", &[("diff.renames", "copies")]),
        (
            "a rename limit that cuts the search short",
            &[("diff.renames", "true"), ("diff.renameLimit", "1")],
        ),
    ];
    for (name, config) in configurations {
        let repo = staged_rewrites(config);
        let records = porcelain_records(&repo);
        for path in PATHS {
            let answer = ask(&repo, path, WorkingTreeDiff::Staged);
            match (record_for(&records, path), &answer) {
                (None, None) => {}
                (Some(theirs), Some(ours)) => {
                    assert_eq!(&ours.file, theirs, "{name}: {path} is not git's record");
                    same_as_git(&repo, path, WorkingTreeDiff::Staged, &answer);
                }
                (theirs, ours) => panic!("{name}: {path}: git says {theirs:?}, Cairn {ours:?}"),
            }
        }

        // The configurations are not all one answer: each pairs what it should, so the
        // comparisons above decided something.
        let status = |path: &str| record_for(&records, path).map(|file| file.status);
        let renamed = matches!(status("new.txt"), Some(ChangeStatus::Renamed(_)));
        let copied = matches!(status("copy.txt"), Some(ChangeStatus::Copied(_)));
        let expected = match name {
            "diff.renames=false" | "a rename limit that cuts the search short" => (false, false),
            "diff.renames=copies" => (true, true),
            _ => (true, false),
        };
        assert_eq!((renamed, copied), expected, "{name}: git paired otherwise");
        assert_eq!(
            status("ita"),
            None,
            "{name}: git listed the intent-to-add entry"
        );
        assert_eq!(
            status("empty"),
            Some(ChangeStatus::Deleted),
            "{name}: git paired the deleted empty file"
        );
    }
}

/// Why the oracle above is the whole index: a pathspec of the destination alone shows git
/// an addition, under every setting. Without this, C7's oracle could quietly be the one-path
/// diff, which pairs nothing and agrees with the bug R2.6 fixed.
#[test]
fn git_diff_cached_of_one_path_pairs_nothing() {
    let repo = staged_rewrites(&[]);
    let whole = repo.git(&["diff", "--cached", "--name-status"]);
    assert!(
        whole
            .lines()
            .any(|line| line.starts_with('R') && line.ends_with("new.txt")),
        "git did not pair the rename over the whole index: {whole}"
    );
    let one = repo.git(&["diff", "--cached", "--name-status", "--", "new.txt"]);
    assert_eq!(one.trim(), "A\tnew.txt", "a one-path diff paired after all");
}
