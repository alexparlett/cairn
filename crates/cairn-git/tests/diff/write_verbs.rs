//! staging-and-commit's write verbs against real `git` (R3, R1.4): C2's discard and delete
//! halves, C5, C6, C8's engine half and C9's effect, on the host's git and, through
//! `scripts/git-floor.sh`, on 2.30.9 and 2.32.7. The arguments each verb is given are pinned
//! separately, against a recording `git`, in `crates/cairn-git/src/ops/stage.rs` and
//! `discard.rs`; here is what they do to a repository.
//!
//! Every oracle is git's own — `git status`, `git ls-files -s`, `git show :<path>`, the bytes
//! on disk — or the model's reference applier, never the code under test.

use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;

use cairn_git::{
    CancelSignal, ContentOptions, Error, Refusal, Repository, SharedRepository, WorkingTreeDiff,
    ops,
};
use cairn_model::{Confirmed, DiffContent, FileDiff, RepoPath, Selection, apply_patch};

use super::repositories::Repo;
use super::{git, ok};

fn engine(repo: &Repo) -> Repository {
    ok(Repository::discover(repo.path()), "the fixture opens")
}

fn diff_of(repo: &Repo, path: &str, which: WorkingTreeDiff) -> FileDiff {
    let answer = ok(
        engine(repo).working_tree_diff(
            git(),
            &RepoPath::new(path),
            which,
            &ContentOptions::default(),
            &CancelSignal::new(),
        ),
        &format!("{path}'s {which:?} diff"),
    );
    answer.unwrap_or_else(|| panic!("{path} has no {which:?} diff"))
}

/// The first change of `diff`'s text, every line of it.
fn first_change(diff: &FileDiff) -> Selection {
    let text = diff
        .text()
        .unwrap_or_else(|| panic!("{} drew no text: {:?}", diff.file.new_path, diff.content));
    let mut selection = Selection::empty();
    selection.select_change(&text.changes()[0]);
    selection
}

/// What `git status` lists, one `XY path` per entry, untracked files each listed.
fn status(repo: &Repo) -> Vec<String> {
    repo.git(&["status", "--porcelain=v1", "-uall"])
        .lines()
        .map(str::to_owned)
        .collect()
}

/// The index's bytes for `path`, or `None` where it holds no entry.
fn staged(repo: &Repo, path: &str) -> Option<Vec<u8>> {
    repo.try_git(&["show", &format!(":{path}")], &[], None)
        .ok()
        .map(String::into_bytes)
}

fn on_disk(repo: &Repo, path: &str) -> Option<Vec<u8>> {
    std::fs::read(repo.path().join(path)).ok()
}

fn index_bytes(repo: &Repo) -> Vec<u8> {
    std::fs::read(repo.path().join(".git/index")).unwrap_or_default()
}

/// Every file under `dir`, with its bytes, for a byte-for-byte before-and-after.
fn snapshot(dir: &Path) -> Vec<(std::path::PathBuf, Vec<u8>)> {
    let mut found = Vec::new();
    let mut pending = vec![dir.to_owned()];
    while let Some(directory) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                pending.push(path);
            } else {
                found.push((path.clone(), std::fs::read(&path).unwrap_or_default()));
            }
        }
    }
    found.sort();
    found
}

/// Twenty numbered lines, `edit` applied to each.
fn lines(edit: impl Fn(usize) -> String) -> String {
    (0..20).map(|n| format!("{}\n", edit(n))).collect()
}

fn base(n: usize) -> String {
    format!("line {n:02}")
}

/// `file.txt` committed as twenty lines, with line 12 edited in the working tree.
fn one_unstaged_edit(name: &str) -> Repo {
    let repo = Repo::new(name);
    repo.write("file.txt", lines(base).as_bytes());
    repo.commit("base");
    repo.write(
        "file.txt",
        lines(|n| if n == 12 { "work 12".into() } else { base(n) }).as_bytes(),
    );
    repo
}

fn is_refused(outcome: &Result<impl std::fmt::Debug, Error>, refusal: Refusal) -> bool {
    matches!(outcome, Err(Error::Refused { why, .. }) if *why == refusal)
}

// --- C6: a stale patch writes nothing, where git apply alone would land it at an offset ---

/// C6 for a stage: the index moved under the diff by three lines inserted above the change,
/// so `git apply --cached` of the very patch still applies — at an offset, on the moved
/// lines (spike E6), which the test proves first — and the verb refuses, naming the path,
/// with the index byte-identical.
#[test]
fn a_stale_stage_writes_nothing_where_git_apply_would_land_it_at_an_offset() {
    let repo = one_unstaged_edit("c6-stage");
    let drawn = diff_of(&repo, "file.txt", WorkingTreeDiff::Unstaged);
    let selection = first_change(&drawn);
    let patch = cairn_model::action_patch(cairn_model::PatchAction::Stage, &drawn, &selection);

    // The index moves: three lines inserted at the top, the working tree left as drawn.
    let worktree = on_disk(&repo, "file.txt").unwrap_or_default();
    repo.write(
        "file.txt",
        format!("new 1\nnew 2\nnew 3\n{}", lines(base)).as_bytes(),
    );
    repo.git(&["add", "file.txt"]);
    repo.write("file.txt", &worktree);
    let index = index_bytes(&repo);

    // git apply alone lands it, three lines down.
    let (status, _, stderr) = repo.run(
        &["apply", "--cached", "--whitespace=nowarn", "-v", "-"],
        &[],
        Some(patch.as_bytes()),
    );
    assert!(
        status.success() && stderr.contains("offset 3"),
        "git apply did not land the stale patch at an offset, so this decides nothing: \
         {status} {stderr}"
    );
    std::fs::write(repo.path().join(".git/index"), &index).unwrap_or_else(|e| panic!("{e}"));

    let outcome = ops::stage_lines(git(), &engine(&repo), &drawn, &selection, None);
    assert!(
        matches!(&outcome, Err(Error::ChangedSinceRead { path }) if path == "file.txt"),
        "{outcome:?}"
    );
    assert_eq!(index_bytes(&repo), index, "a refused stage wrote the index");
}

/// C6 for an unstage: the entry was restaged since the staged diff was read.
#[test]
fn a_stale_unstage_writes_nothing() {
    let repo = Repo::new("c6-unstage");
    repo.write("file.txt", lines(base).as_bytes());
    repo.commit("base");
    repo.write(
        "file.txt",
        lines(|n| if n == 12 { "staged 12".into() } else { base(n) }).as_bytes(),
    );
    repo.git(&["add", "file.txt"]);
    let drawn = diff_of(&repo, "file.txt", WorkingTreeDiff::Staged);
    repo.write(
        "file.txt",
        format!(
            "top\n{}",
            lines(|n| if n == 12 { "staged 12".into() } else { base(n) })
        )
        .as_bytes(),
    );
    repo.git(&["add", "file.txt"]);
    let index = index_bytes(&repo);
    let outcome = ops::unstage_lines(git(), &engine(&repo), &drawn, &first_change(&drawn), None);
    assert!(
        matches!(&outcome, Err(Error::ChangedSinceRead { path }) if path == "file.txt"),
        "{outcome:?}"
    );
    assert_eq!(index_bytes(&repo), index);
}

/// C6 and C2 for a discard of lines: the file moved after the confirmation by lines above
/// the change, where `git apply` of the discard's patch would still land at an offset — and
/// the discard refuses, the file's bytes untouched.
#[test]
fn a_stale_discard_writes_nothing_where_git_apply_would_land_it_at_an_offset() {
    let repo = one_unstaged_edit("c6-discard");
    let drawn = diff_of(&repo, "file.txt", WorkingTreeDiff::Unstaged);
    let selection = first_change(&drawn);
    let patch = cairn_model::action_patch(cairn_model::PatchAction::Discard, &drawn, &selection);
    let consequence = ok(
        ops::discard_lines_consequence(git(), &engine(&repo), &drawn, selection),
        "the consequence",
    );
    let moved = format!(
        "typed 1\ntyped 2\n{}",
        lines(|n| if n == 12 { "work 12".into() } else { base(n) })
    );
    repo.write("file.txt", moved.as_bytes());

    let (status, _, stderr) = repo.run(
        &["apply", "--whitespace=nowarn", "--check", "-v", "-"],
        &[],
        Some(patch.as_bytes()),
    );
    assert!(
        status.success() && stderr.contains("offset 2"),
        "git apply would not land the stale discard at an offset, so this decides nothing: \
         {status} {stderr}"
    );
    let outcome = ops::discard_lines(git(), &engine(&repo), Confirmed::by_user(consequence), None);
    assert!(
        matches!(&outcome, Err(Error::ChangedSinceConfirmed { path }) if path == "file.txt"),
        "{outcome:?}"
    );
    assert_eq!(on_disk(&repo, "file.txt"), Some(moved.into_bytes()));
}

// --- C2: each destructive operation refuses when what it names moved ---

/// C2 for a discard of lines, by each way the state can move: the file edited, its line
/// endings alone changed under `core.autocrlf` (which git's form does not show — phase 01's
/// QA item 25), and the index entry restaged. Each refuses and writes nothing; and the
/// computation of the consequence wrote nothing either (no object: `hash-object` has no `-w`).
#[test]
fn a_discard_of_lines_refuses_whatever_moved_after_the_confirmation() {
    type Move = fn(&Repo);
    let moves: [(&str, Move); 5] = [
        ("its mode alone", |repo| repo.chmod("file.txt", 0o755)),
        // The bytes and the index unchanged, git's form of the file moved: what only the
        // git-form comparison sees (QA item 6).
        ("its git form alone, by a filter attribute", |repo| {
            repo.config("filter.rot.clean", "tr a-zA-Z n-za-mN-ZA-M");
            repo.config("filter.rot.smudge", "tr a-zA-Z n-za-mN-ZA-M");
            repo.write(".gitattributes", b"file.txt filter=rot\n");
        }),
        ("an edit", |repo| {
            repo.write(
                "file.txt",
                lines(|n| {
                    if n == 12 {
                        "typed again".into()
                    } else {
                        base(n)
                    }
                })
                .as_bytes(),
            );
        }),
        ("the line endings alone", |repo| {
            let crlf =
                lines(|n| if n == 12 { "work 12".into() } else { base(n) }).replace('\n', "\r\n");
            repo.write("file.txt", crlf.as_bytes());
        }),
        ("the entry restaged", |repo| {
            repo.git(&["add", "file.txt"]);
        }),
    ];
    for (what, moved) in moves {
        let repo = one_unstaged_edit("c2-lines");
        repo.config("core.autocrlf", "true");
        repo.config("core.safecrlf", "false");
        let drawn = diff_of(&repo, "file.txt", WorkingTreeDiff::Unstaged);
        let objects = snapshot(&repo.path().join(".git/objects"));
        let consequence = ok(
            ops::discard_lines_consequence(git(), &engine(&repo), &drawn, first_change(&drawn)),
            what,
        );
        assert_eq!(
            snapshot(&repo.path().join(".git/objects")),
            objects,
            "{what}: computing the consequence wrote an object"
        );
        moved(&repo);
        let before = snapshot(repo.path());
        let outcome =
            ops::discard_lines(git(), &engine(&repo), Confirmed::by_user(consequence), None);
        assert!(
            matches!(&outcome, Err(Error::ChangedSinceConfirmed { path }) if path == "file.txt"),
            "{what}: {outcome:?}"
        );
        assert_eq!(
            snapshot(repo.path()),
            before,
            "{what}: a refused discard wrote"
        );
    }
}

/// A repository with a modified `a.txt`, an untracked `dir/u.txt` and nothing else changed:
/// a discard of both is confirmed before each move below.
fn modified_and_untracked(name: &str) -> Repo {
    let repo = Repo::new(name);
    repo.write("a.txt", lines(base).as_bytes());
    repo.commit("base");
    repo.write(
        "a.txt",
        lines(|n| if n == 4 { "work 4".into() } else { base(n) }).as_bytes(),
    );
    repo.write("dir/u.txt", b"untracked\n");
    repo
}

fn both() -> [RepoPath; 2] {
    [RepoPath::new("a.txt"), RepoPath::new("dir/u.txt")]
}

/// C2 for a discard of files, by every way a confirmed file can move: the tracked file
/// edited or its line endings alone changed, its entry restaged, the untracked file edited,
/// removed, replaced by a directory, or staged. Each refuses before the first write, so
/// nothing at all is written — not the file that did not move either.
#[test]
fn a_discard_of_files_refuses_whatever_moved_after_the_confirmation() {
    type Move = fn(&Repo);
    let moves: [(&str, Move); 9] = [
        ("the tracked file's mode alone", |repo| {
            repo.chmod("a.txt", 0o755)
        }),
        ("the untracked file's mode alone", |repo| {
            repo.chmod("dir/u.txt", 0o755)
        }),
        ("the tracked file edited", |repo| {
            repo.write("a.txt", b"typed\n");
        }),
        ("the tracked file's line endings alone", |repo| {
            let crlf =
                lines(|n| if n == 4 { "work 4".into() } else { base(n) }).replace('\n', "\r\n");
            repo.write("a.txt", crlf.as_bytes());
        }),
        ("the tracked file restaged", |repo| {
            repo.git(&["add", "a.txt"]);
        }),
        ("the untracked file edited", |repo| {
            repo.write("dir/u.txt", b"untracked, and typed\n");
        }),
        ("the untracked file removed", |repo| {
            repo.remove("dir/u.txt");
        }),
        ("the untracked file replaced by a directory", |repo| {
            repo.remove("dir/u.txt");
            repo.write("dir/u.txt/inside.txt", b"a file the prompt never named\n");
        }),
        ("the untracked file staged", |repo| {
            repo.git(&["add", "dir/u.txt"]);
        }),
    ];
    for (what, moved) in moves {
        let repo = modified_and_untracked("c2-files");
        repo.config("core.autocrlf", "true");
        repo.config("core.safecrlf", "false");
        let consequence = ok(
            ops::discard_files_consequence(git(), &engine(&repo), &both()),
            what,
        );
        moved(&repo);
        let before = snapshot(repo.path());
        let outcome =
            ops::discard_files(git(), &engine(&repo), Confirmed::by_user(consequence), None);
        assert!(
            matches!(&outcome, Err(Error::ChangedSinceConfirmed { .. })),
            "{what}: {outcome:?}"
        );
        assert_eq!(
            snapshot(repo.path()),
            before,
            "{what}: a refused discard wrote"
        );
    }
}

/// C2 as amended (the user's decision 3 of 2026-10-08): Cairn deletes only the files `git
/// status` listed, one per file and never a directory (no `-d`), so an untracked file added
/// beside a confirmed one is no part of what the prompt counted and is never taken, and the
/// discard of the confirmed file goes ahead. What stands at a confirmed path's directory is
/// refused instead (`a_file_where_a_deleted_files_directory_was_is_never_destroyed`,
/// `a_symlinked_parent_is_never_followed_out_of_the_working_tree`).
#[test]
fn a_file_added_beside_a_confirmed_deletion_is_never_taken() {
    let repo = modified_and_untracked("c2-beside");
    let consequence = ok(
        ops::discard_files_consequence(git(), &engine(&repo), &[RepoPath::new("dir/u.txt")]),
        "the consequence",
    );
    repo.write("dir/newcomer.txt", b"added after the confirmation\n");
    ok(
        ops::discard_files(git(), &engine(&repo), Confirmed::by_user(consequence), None),
        "the discard",
    );
    assert_eq!(on_disk(&repo, "dir/u.txt"), None);
    assert_eq!(
        on_disk(&repo, "dir/newcomer.txt"),
        Some(b"added after the confirmation\n".to_vec())
    );
}

// --- C5: what is staged is what was selected, whatever apply's settings say ---

/// C5: under `apply.whitespace=fix` and `=error`, staging lines with trailing whitespace
/// stages them byte for byte — `--whitespace=nowarn` outranks both — where git's default
/// handling under `fix` would strip them (proved first, so the test decides something).
#[test]
fn trailing_whitespace_is_staged_byte_for_byte_whatever_apply_whitespace_says() {
    for setting in ["fix", "error"] {
        let repo = Repo::new("c5-whitespace");
        repo.write("file.txt", lines(base).as_bytes());
        repo.commit("base");
        let edited = lines(|n| {
            if n == 7 {
                "trailing   \t".into()
            } else {
                base(n)
            }
        });
        repo.write("file.txt", edited.as_bytes());
        repo.config("apply.whitespace", setting);
        let drawn = diff_of(&repo, "file.txt", WorkingTreeDiff::Unstaged);
        let selection = first_change(&drawn);
        let patch = cairn_model::action_patch(cairn_model::PatchAction::Stage, &drawn, &selection);
        let index = index_bytes(&repo);
        let (status, _, _) = repo.run(&["apply", "--cached", "-"], &[], Some(patch.as_bytes()));
        let by_default = staged(&repo, "file.txt");
        assert!(
            !status.success() || by_default.as_deref() != Some(edited.as_bytes()),
            "apply.whitespace={setting} changed nothing by default, so this decides nothing"
        );
        std::fs::write(repo.path().join(".git/index"), &index).unwrap_or_else(|e| panic!("{e}"));

        ok(
            ops::stage_lines(git(), &engine(&repo), &drawn, &selection, None),
            setting,
        );
        assert_eq!(
            staged(&repo, "file.txt").as_deref(),
            Some(edited.as_bytes()),
            "apply.whitespace={setting}: other bytes were staged"
        );
    }
}

/// C5's second half, measured in phase 03 before it was pinned or not: with
/// `apply.ignoreWhitespace=change` set, staging part of a diff among lines that differ only
/// in whitespace stages exactly the selected lines — the reference applier's answer over the
/// index's side — so no `-c apply.ignoreWhitespace=false` is pinned. (Over 400 selections on
/// 2.30.9, 2.32.7 and 2.56.0 the setting changed nothing a fresh patch stages; it lets a
/// patch land on context that moved by whitespace, which the stale check refuses first.)
#[test]
fn apply_ignore_whitespace_stages_exactly_the_selected_lines() {
    let repo = Repo::new("c5-ignore-whitespace");
    let index_side = "a\n a\na \n\ta\nb\nb  \nc\n  c\nd\nd\nd \ne\nf\nf\t\ng\ng\n";
    repo.write("file.txt", index_side.as_bytes());
    repo.commit("base");
    repo.write(
        "file.txt",
        b"a\n  a\na \n\ta\nX\nb\nb  \nc\nc \n  c\nd\nd\nY\nd \ne\nf \nf\t\ng\nZ\ng\n",
    );
    repo.config("apply.ignoreWhitespace", "change");
    let drawn = diff_of(&repo, "file.txt", WorkingTreeDiff::Unstaged);
    let text = drawn
        .text()
        .unwrap_or_else(|| panic!("no text: {:?}", drawn.content));
    assert!(
        text.changes().len() >= 4,
        "too few changes to select part of"
    );
    let mut selection = Selection::empty();
    for change in text.changes().iter().step_by(2) {
        selection.select_change(change);
    }
    let patch = cairn_model::action_patch(cairn_model::PatchAction::Stage, &drawn, &selection);
    let expected = apply_patch(index_side.as_bytes(), patch.as_bytes())
        .unwrap_or_else(|e| panic!("the reference applier refused it: {e}"));
    ok(
        ops::stage_lines(git(), &engine(&repo), &drawn, &selection, None),
        "the stage",
    );
    assert_eq!(staged(&repo, "file.txt"), Some(expected));
}

// --- C8: file verbs leave what git status reports ---

/// C8: a modification, a new file and a deletion staged by `git add`, then unstaged by
/// `git reset -q`, each leaving what `git status` says.
#[test]
fn staging_and_unstaging_files_leave_what_git_status_reports() {
    let repo = Repo::new("c8-files");
    repo.write("modified.txt", b"one\n");
    repo.write("deleted.txt", b"gone soon\n");
    repo.commit("base");
    repo.write("modified.txt", b"two\n");
    repo.remove("deleted.txt");
    repo.write("new.txt", b"new\n");
    let paths = [
        RepoPath::new("modified.txt"),
        RepoPath::new("deleted.txt"),
        RepoPath::new("new.txt"),
    ];
    let performed = ok(
        ops::stage_files(git(), &engine(&repo), &paths, None),
        "the stage",
    );
    assert!(performed.invalidated().index && !performed.invalidated().working_tree);
    assert_eq!(
        status(&repo),
        ["D  deleted.txt", "M  modified.txt", "A  new.txt"]
    );
    ok(
        ops::unstage_files(git(), &engine(&repo), &paths, &ops::UnstageTo::Head, None),
        "the unstage",
    );
    assert_eq!(
        status(&repo),
        [" D deleted.txt", " M modified.txt", "?? new.txt"]
    );
}

/// C8: on an unborn branch, where `git restore --staged` fails, a staged file unstages back
/// to untracked.
#[test]
fn unstaging_on_an_unborn_branch_leaves_the_file_untracked() {
    let repo = Repo::new("c8-unborn");
    repo.write("a.txt", b"a\n");
    repo.write("b.txt", b"b\n");
    repo.git(&["add", "a.txt", "b.txt"]);
    ok(
        ops::unstage_files(
            git(),
            &engine(&repo),
            &[RepoPath::new("a.txt")],
            &ops::UnstageTo::Head,
            None,
        ),
        "the unstage",
    );
    assert_eq!(status(&repo), ["A  b.txt", "?? a.txt"]);
}

/// C8 and C13: out of an amend, a file unstages back to `HEAD`'s parent's entry (R6.3); out of
/// the amend of a root commit, which has no parent, its entry is removed (`git rm --cached
/// -f`), an edited one's too.
#[test]
fn unstaging_out_of_an_amend_puts_back_the_parents_entry_or_none() {
    let repo = Repo::new("c8-amend");
    repo.write("file.txt", b"first\n");
    let first = repo.commit("first");
    repo.write("file.txt", b"second\n");
    repo.commit("second");
    repo.write("file.txt", b"third\n");
    repo.git(&["add", "file.txt"]);
    ok(
        ops::unstage_files(
            git(),
            &engine(&repo),
            &[RepoPath::new("file.txt")],
            &ops::UnstageTo::Commit(first),
            None,
        ),
        "the unstage out of an amend",
    );
    assert_eq!(staged(&repo, "file.txt"), Some(b"first\n".to_vec()));

    let root = Repo::new("c8-root-amend");
    root.write("file.txt", b"root\n");
    root.commit("root");
    ok(
        ops::unstage_files(
            git(),
            &engine(&root),
            &[RepoPath::new("file.txt")],
            &ops::UnstageTo::Nothing,
            None,
        ),
        "the unstage out of a root commit's amend",
    );
    assert_eq!(staged(&root, "file.txt"), None);
    assert_eq!(on_disk(&root, "file.txt"), Some(b"root\n".to_vec()));

    // The path an amend's staged list shows edited: staged content that is neither `HEAD`'s
    // nor the file's, which `git rm --cached` without `-f` refuses. Unstaged, its entry goes
    // and the file stays as it is, as `git reset` would leave it.
    let edited = Repo::new("c8-root-amend-edited");
    edited.write("file.txt", b"root\n");
    edited.commit("root");
    edited.write("file.txt", b"staged\n");
    edited.git(&["add", "file.txt"]);
    edited.write("file.txt", b"working\n");
    ok(
        ops::unstage_files(
            git(),
            &engine(&edited),
            &[RepoPath::new("file.txt")],
            &ops::UnstageTo::Nothing,
            None,
        ),
        "the unstage of an edited path out of a root commit's amend",
    );
    assert_eq!(staged(&edited, "file.txt"), None);
    assert_eq!(on_disk(&edited, "file.txt"), Some(b"working\n".to_vec()));
}

/// C8 and R3.4: a staged rename unstages whole when both its paths are named, and one path
/// named unstages that side alone, as `git reset -- <path>` does.
#[test]
fn a_staged_rename_unstages_whole_by_both_paths_and_one_path_alone_by_its_own() {
    let make = || {
        let repo = Repo::new("c8-rename");
        repo.write("old.txt", lines(base).as_bytes());
        repo.commit("base");
        repo.git(&["mv", "old.txt", "new.txt"]);
        repo
    };
    let repo = make();
    ok(
        ops::unstage_files(
            git(),
            &engine(&repo),
            &[RepoPath::new("old.txt"), RepoPath::new("new.txt")],
            &ops::UnstageTo::Head,
            None,
        ),
        "the whole unstage",
    );
    assert_eq!(status(&repo), [" D old.txt", "?? new.txt"]);
    let repo = make();
    ok(
        ops::unstage_files(
            git(),
            &engine(&repo),
            &[RepoPath::new("new.txt")],
            &ops::UnstageTo::Head,
            None,
        ),
        "one side",
    );
    assert_eq!(status(&repo), ["D  old.txt", "?? new.txt"]);
}

/// Phase 02's QA item 8: where `status.renames` is off and `diff.renames` on, status lists a
/// rename's source as a row of its own, while that row's staged diff is the rename as `git
/// diff --cached` pairs it. Its lines unstage at the NEW path — the source's entry is never
/// touched — and the source row's own whole-file unstage restores the source alone.
#[test]
fn a_rename_sources_row_unstages_its_lines_at_the_new_path() {
    let repo = Repo::new("c8-rename-source");
    repo.config("status.renames", "false");
    repo.write("old.txt", lines(base).as_bytes());
    repo.commit("base");
    repo.git(&["mv", "old.txt", "new.txt"]);
    repo.write(
        "new.txt",
        lines(|n| if n == 9 { "renamed 9".into() } else { base(n) }).as_bytes(),
    );
    repo.git(&["add", "new.txt"]);
    assert_eq!(status(&repo), ["A  new.txt", "D  old.txt"]);

    let drawn = diff_of(&repo, "old.txt", WorkingTreeDiff::Staged);
    assert!(
        drawn.file.is_rename(),
        "the source row's diff is not the rename: {drawn:?}"
    );
    ok(
        ops::unstage_lines(git(), &engine(&repo), &drawn, &first_change(&drawn), None),
        "the lines",
    );
    assert_eq!(staged(&repo, "new.txt"), Some(lines(base).into_bytes()));
    assert_eq!(
        staged(&repo, "old.txt"),
        None,
        "the source's entry came back"
    );

    ok(
        ops::unstage_files(
            git(),
            &engine(&repo),
            &[RepoPath::new("old.txt")],
            &ops::UnstageTo::Head,
            None,
        ),
        "the source row",
    );
    assert_eq!(staged(&repo, "old.txt"), Some(lines(base).into_bytes()));
    assert!(
        staged(&repo, "new.txt").is_some(),
        "the new path was unstaged too"
    );
}

/// C8: a discard of files restores each tracked file from the index — a modification, and a
/// file deleted in the working tree, which comes back — and deletes exactly the untracked
/// files named, one `git clean -f --` with no `-d`; the prompt counts each kind, and the
/// record quotes it.
#[test]
fn a_discard_of_files_restores_the_tracked_and_deletes_exactly_the_untracked() {
    let repo = Repo::new("c8-discard");
    repo.write("a.txt", lines(base).as_bytes());
    repo.write("b.txt", b"restored\n");
    repo.commit("base");
    repo.write(
        "a.txt",
        lines(|n| if n == 4 { "work 4".into() } else { base(n) }).as_bytes(),
    );
    repo.remove("b.txt");
    repo.write("dir/u1.txt", b"twelve bytes");
    repo.write("dir/u2.txt", b"kept\n");
    let shared = ok(SharedRepository::discover(repo.path()), "the fixture opens");
    let worker = shared.to_worker();
    let paths = [
        RepoPath::new("a.txt"),
        RepoPath::new("b.txt"),
        RepoPath::new("dir/u1.txt"),
    ];
    let consequence = ok(
        ops::discard_files_consequence(git(), &worker, &paths),
        "the consequence",
    );
    let prompt = consequence.prompt();
    assert_eq!(
        prompt,
        "Do you want to discard the changes in 3 files? 1 modified (2 lines), 1 deleted file \
         restored, 1 untracked file deleted (12 bytes). You can't undo this action."
    );
    let performed = ok(
        ops::discard_files(git(), &worker, Confirmed::by_user(consequence), None),
        "the discard",
    );
    assert_eq!(performed.acknowledged(), Some(prompt.as_str()));
    assert_eq!(on_disk(&repo, "a.txt"), Some(lines(base).into_bytes()));
    assert_eq!(on_disk(&repo, "b.txt"), Some(b"restored\n".to_vec()));
    assert_eq!(on_disk(&repo, "dir/u1.txt"), None);
    assert_eq!(status(&repo), ["?? dir/u2.txt"]);
    let cleans: Vec<Vec<String>> = shared
        .command_log()
        .into_iter()
        .map(|record| record.arguments)
        .filter(|arguments| arguments.iter().any(|argument| argument == "clean"))
        .collect();
    assert_eq!(
        cleans,
        [vec![
            "--literal-pathspecs",
            "clean",
            "-f",
            "-q",
            "--",
            "dir/u1.txt"
        ]]
    );
}

/// C8 and R3.5: a list of untracked files past the bound is deleted by several `git clean`s,
/// every file named once — and the one re-check comes before the first of them, so a file
/// at the end of the list that moved refuses the whole discard with nothing deleted.
#[test]
fn a_long_list_is_deleted_in_batches_after_one_recheck() {
    let repo = Repo::new("c8-batches");
    repo.write("tracked.txt", b"t\n");
    repo.commit("base");
    let directory = "d".repeat(100);
    let paths: Vec<RepoPath> = (0..700)
        .map(|n| RepoPath::new(format!("{directory}/untracked file {n:04}.txt")))
        .collect();
    for path in &paths {
        repo.write(&path.to_string(), b"x\n");
    }
    let per_path: usize = paths.iter().map(|path| path.as_bytes().len() + 9).sum();
    assert!(
        per_path > ops::CLEAN_ARGUMENT_BYTES,
        "the list fits one invocation, so this decides nothing"
    );

    let consequence = ok(
        ops::discard_files_consequence(git(), &engine(&repo), &paths),
        "the consequence",
    );
    let last = paths[paths.len() - 1].to_string();
    repo.write(&last, b"moved\n");
    let outcome = ops::discard_files(git(), &engine(&repo), Confirmed::by_user(consequence), None);
    assert!(matches!(&outcome, Err(Error::ChangedSinceConfirmed { path }) if *path == last));
    assert!(
        paths
            .iter()
            .all(|path| on_disk(&repo, &path.to_string()).is_some()),
        "a file was deleted before the re-check refused"
    );

    let shared = ok(SharedRepository::discover(repo.path()), "the fixture opens");
    let worker = shared.to_worker();
    let consequence = ok(
        ops::discard_files_consequence(git(), &worker, &paths),
        "the consequence again",
    );
    ok(
        ops::discard_files(git(), &worker, Confirmed::by_user(consequence), None),
        "the discard",
    );
    assert_eq!(status(&repo), Vec::<String>::new());
    let cleaned: Vec<Vec<String>> = shared
        .command_log()
        .into_iter()
        .map(|record| record.arguments)
        .filter(|arguments| arguments.iter().any(|argument| argument == "clean"))
        .collect();
    assert!(cleaned.len() >= 2, "{} invocations", cleaned.len());
    let mut named: Vec<String> = cleaned
        .iter()
        .flat_map(|arguments| {
            assert_eq!(
                arguments[..5],
                ["--literal-pathspecs", "clean", "-f", "-q", "--"]
            );
            arguments[5..].to_vec()
        })
        .collect();
    named.sort();
    let mut expected: Vec<String> = paths.iter().map(ToString::to_string).collect();
    expected.sort();
    assert_eq!(named, expected, "a path was lost or named twice");
}

/// C8 and R3.5: a repository nested in the working tree — the one directory `git status`
/// lists whole — is refused before any confirmation is offered, by either spelling, and
/// stays; any other directory is refused as not a file.
#[test]
fn a_nested_repository_is_refused_before_any_confirmation() {
    let repo = Repo::new("c8-nested");
    repo.write("tracked.txt", b"t\n");
    repo.commit("base");
    let nested = Repo::borrowed(&repo.path().join("nested"));
    std::fs::create_dir_all(nested.path()).unwrap_or_else(|e| panic!("{e}"));
    nested.git(&["init", "--quiet", "."]);
    nested.write("history.txt", b"commits only it has\n");
    nested.commit("only here");
    assert_eq!(status(&repo), ["?? nested/"]);
    for spelling in ["nested/", "nested"] {
        let outcome =
            ops::discard_files_consequence(git(), &engine(&repo), &[RepoPath::new(spelling)]);
        assert!(
            is_refused(&outcome, Refusal::NestedRepository),
            "{spelling}: {outcome:?}"
        );
    }
    assert!(nested.path().join(".git").is_dir());
    repo.write("plain/file.txt", b"p\n");
    let outcome = ops::discard_files_consequence(git(), &engine(&repo), &[RepoPath::new("plain/")]);
    assert!(is_refused(&outcome, Refusal::NotAFile), "{outcome:?}");
}

/// R8.1 and the QA brief: a collapsed untracked-directory row — the one directory `git status`
/// still lists whole once every untracked file is listed, a nested repository — staged by the
/// path its row shows (`nested/`) adds what `git add` adds for it and nothing else: the
/// repository as a gitlink at its own `HEAD`, read back from the index; the untracked file
/// beside it stays untracked and nothing inside the nested repository is added. Caught by: a
/// row's stage that globs, recurses into the nested repository's files, or takes its
/// neighbour.
#[test]
fn a_nested_repositorys_row_stages_exactly_what_git_add_adds_for_it() {
    let repo = Repo::new("r8-nested-stage");
    repo.write("tracked.txt", b"t\n");
    repo.commit("base");
    let nested = Repo::borrowed(&repo.path().join("nested"));
    std::fs::create_dir_all(nested.path()).unwrap_or_else(|e| panic!("{e}"));
    nested.git(&["init", "--quiet", "."]);
    nested.write("history.txt", b"commits only it has\n");
    let head = nested.commit("only here");
    repo.write("other/beside.txt", b"untracked\n");
    assert_eq!(status(&repo), ["?? nested/", "?? other/beside.txt"]);
    ok(
        ops::stage_files(git(), &engine(&repo), &[RepoPath::new("nested/")], None),
        "staging the nested repository's row",
    );
    assert_eq!(
        repo.git(&["ls-files", "--stage"])
            .lines()
            .collect::<Vec<_>>(),
        [
            format!("160000 {head} 0\tnested"),
            format!(
                "100644 {} 0\ttracked.txt",
                repo.git(&["rev-parse", ":tracked.txt"]).trim()
            ),
        ]
    );
    assert_eq!(status(&repo), ["A  nested", "?? other/beside.txt"]);
}

/// C8 and R3.6: staged changes are never discarded — a path whose only change is staged has
/// nothing a discard takes, and is refused before any prompt.
#[test]
fn staged_changes_are_never_discarded() {
    let repo = Repo::new("c8-staged-only");
    repo.write("file.txt", b"one\n");
    repo.commit("base");
    repo.write("file.txt", b"two\n");
    repo.git(&["add", "file.txt"]);
    let outcome =
        ops::discard_files_consequence(git(), &engine(&repo), &[RepoPath::new("file.txt")]);
    assert!(
        is_refused(&outcome, Refusal::NoUnstagedChange),
        "{outcome:?}"
    );
}

/// C8 and R3.10: no verb discards a submodule — its row's file discard is refused, and so is
/// a discard of lines of its diff — while `git add` stages its new commit whole.
#[test]
fn a_submodule_is_never_discarded() {
    let inner = Repo::new("c8-submodule-inner");
    inner.write("x.txt", b"one\n");
    let first = inner.commit("one");
    inner.write("x.txt", b"two\n");
    inner.commit("two");
    let repo = Repo::new("c8-submodule");
    repo.write("t.txt", b"t\n");
    repo.commit("base");
    let url = inner.path().to_string_lossy().into_owned();
    repo.git(&[
        "-c",
        "protocol.file.allow=always",
        "submodule",
        "add",
        "--quiet",
        &url,
        "sub",
    ]);
    repo.commit("add the submodule");
    Repo::borrowed(&repo.path().join("sub")).git(&["checkout", "--quiet", &first.to_string()]);
    assert_eq!(status(&repo), [" M sub"]);

    let outcome = ops::discard_files_consequence(git(), &engine(&repo), &[RepoPath::new("sub")]);
    assert!(is_refused(&outcome, Refusal::Submodule), "{outcome:?}");
    let drawn = diff_of(&repo, "sub", WorkingTreeDiff::Unstaged);
    assert!(
        matches!(drawn.content, DiffContent::Submodule { .. }),
        "{:?}",
        drawn.content
    );
    let mut mode = Selection::empty();
    mode.select_mode();
    let outcome = ops::discard_lines_consequence(git(), &engine(&repo), &drawn, mode);
    assert!(is_refused(&outcome, Refusal::Submodule), "{outcome:?}");
    ok(
        ops::stage_files(git(), &engine(&repo), &[RepoPath::new("sub")], None),
        "git add",
    );
    assert_eq!(status(&repo), ["M  sub"]);
}

/// C24's engine half and R3.11: a conflicted path takes no patch and no discard by any verb,
/// and `git add` stages it whole, which marks it resolved.
#[test]
fn a_conflicted_path_takes_no_patch_or_discard_and_add_resolves_it() {
    let repo = Repo::new("c8-conflict");
    repo.write("file.txt", b"base\n");
    repo.commit("base");
    repo.git(&["checkout", "--quiet", "-b", "side"]);
    repo.write("file.txt", b"side\n");
    repo.commit("side");
    repo.git(&["checkout", "--quiet", "main"]);
    repo.write("file.txt", b"main\n");
    repo.commit("main");
    let (merged, _, _) = repo.run(&["merge", "--quiet", "side"], &[], None);
    assert!(!merged.success(), "the merge did not conflict");
    assert_eq!(status(&repo), ["UU file.txt"]);

    let drawn = diff_of(&repo, "file.txt", WorkingTreeDiff::Unstaged);
    let mut every = Selection::empty();
    every.select_mode();
    let path = [RepoPath::new("file.txt")];
    let refusals = [
        ops::stage_lines(git(), &engine(&repo), &drawn, &every, None).map(drop),
        ops::discard_lines_consequence(git(), &engine(&repo), &drawn, every.clone()).map(drop),
        ops::discard_files_consequence(git(), &engine(&repo), &path).map(drop),
    ];
    for outcome in &refusals {
        assert!(is_refused(outcome, Refusal::Conflicted), "{outcome:?}");
    }
    assert_eq!(status(&repo), ["UU file.txt"]);
    ok(
        ops::stage_files(git(), &engine(&repo), &path, None),
        "git add",
    );
    assert_eq!(status(&repo), ["M  file.txt"]);
}

/// Phase 02's carry-forward: every line of a new file is not discarded by patch — that deletes
/// the file — but by the file verb, whose prompt says the file is deleted and cannot be
/// brought back.
#[test]
fn every_line_of_an_untracked_file_is_discarded_by_the_file_verb_which_says_so() {
    let repo = Repo::new("c8-whole-new");
    repo.write("tracked.txt", b"t\n");
    repo.commit("base");
    repo.write("new.txt", b"one\ntwo\n");
    let drawn = diff_of(&repo, "new.txt", WorkingTreeDiff::Untracked);
    let every = Selection::with_every_change(
        drawn
            .text()
            .unwrap_or_else(|| panic!("no text: {:?}", drawn.content)),
    );
    let outcome = ops::discard_lines_consequence(git(), &engine(&repo), &drawn, every);
    assert!(is_refused(&outcome, Refusal::WholeFileOnly), "{outcome:?}");
    let consequence = ok(
        ops::discard_files_consequence(git(), &engine(&repo), &[RepoPath::new("new.txt")]),
        "the file verb's consequence",
    );
    assert_eq!(
        consequence.prompt(),
        "Do you want to discard the changes in new.txt? 1 untracked file deleted (8 bytes). \
         You can't undo this action."
    );
    ok(
        ops::discard_files(git(), &engine(&repo), Confirmed::by_user(consequence), None),
        "the discard",
    );
    assert_eq!(on_disk(&repo, "new.txt"), None);
}

/// A selection of nothing is refused before any prompt: no "Discard 0 Lines" (phase 01's QA
/// item 18), and no list of no files.
#[test]
fn nothing_selected_is_refused_before_any_prompt() {
    let repo = one_unstaged_edit("nothing");
    let drawn = diff_of(&repo, "file.txt", WorkingTreeDiff::Unstaged);
    let mut mode_alone = Selection::empty();
    mode_alone.select_mode();
    for selection in [Selection::empty(), mode_alone] {
        let outcome = ops::discard_lines_consequence(git(), &engine(&repo), &drawn, selection);
        assert!(
            is_refused(&outcome, Refusal::NothingSelected),
            "{outcome:?}"
        );
    }
    let outcome = ops::discard_files_consequence(git(), &engine(&repo), &[]);
    assert!(matches!(outcome, Err(Error::NoPaths)), "{outcome:?}");
}

/// Phase 01's QA item 19 against real git: a mode change selected for discard is named in
/// the prompt with both modes, and the discard puts the mode back with the lines.
#[test]
fn a_mode_change_selected_for_discard_is_named_and_put_back() {
    let repo = one_unstaged_edit("mode");
    repo.chmod("file.txt", 0o755);
    let drawn = diff_of(&repo, "file.txt", WorkingTreeDiff::Unstaged);
    assert!(drawn.file.mode_changed(), "the fixture has no mode change");
    let mut selection = first_change(&drawn);
    selection.select_mode();
    let consequence = ok(
        ops::discard_lines_consequence(git(), &engine(&repo), &drawn, selection),
        "the consequence",
    );
    assert_eq!(
        consequence.prompt(),
        "Do you want to discard 2 lines and the mode change (100644 to 100755) in file.txt? \
         You can't undo this action."
    );
    ok(
        ops::discard_lines(git(), &engine(&repo), Confirmed::by_user(consequence), None),
        "the discard",
    );
    assert_eq!(on_disk(&repo, "file.txt"), Some(lines(base).into_bytes()));
    let mode = std::fs::metadata(repo.path().join("file.txt"))
        .map(|metadata| metadata.permissions().mode() & 0o111)
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(mode, 0, "the mode was not put back");
    assert_eq!(status(&repo), Vec::<String>::new());
}

/// C9's effect: every path is literal — a file named `*.txt` is that file, and a sibling the
/// glob would match is never staged or deleted.
#[test]
fn every_path_is_read_literally_never_as_a_pattern() {
    let repo = Repo::new("c9-literal");
    repo.write("*.txt", b"star\n");
    repo.write("a.txt", b"a\n");
    repo.commit("base");
    repo.write("*.txt", b"star, edited\n");
    repo.write("a.txt", b"a, edited\n");
    ok(
        ops::stage_files(git(), &engine(&repo), &[RepoPath::new("*.txt")], None),
        "the stage",
    );
    assert_eq!(status(&repo), ["M  *.txt", " M a.txt"]);
    repo.write("st*", b"one\n");
    repo.write("stx", b"other\n");
    let consequence = ok(
        ops::discard_files_consequence(git(), &engine(&repo), &[RepoPath::new("st*")]),
        "the consequence",
    );
    ok(
        ops::discard_files(git(), &engine(&repo), Confirmed::by_user(consequence), None),
        "the discard",
    );
    assert_eq!(on_disk(&repo, "st*"), None);
    assert_eq!(on_disk(&repo, "stx"), Some(b"other\n".to_vec()));
}

/// R3.8: a write lists the lock files around it. A stale `index.lock` fails a stage, whose
/// error names it; a discard of an untracked file, which takes no lock, succeeds, and its
/// record lists the lock as present before and after.
#[test]
fn a_write_names_the_lock_files_around_it() {
    let repo = modified_and_untracked("locks");
    let lock = repo.path().join(".git/index.lock");
    std::fs::write(&lock, b"").unwrap_or_else(|e| panic!("{e}"));
    let outcome = ops::stage_files(git(), &engine(&repo), &[RepoPath::new("a.txt")], None);
    match outcome {
        Err(Error::GitFailed { present_locks, .. }) => {
            assert!(
                present_locks
                    .iter()
                    .any(|path| path.ends_with("index.lock"))
            );
        }
        other => panic!("{other:?}"),
    }
    let consequence = ok(
        ops::discard_files_consequence(git(), &engine(&repo), &[RepoPath::new("dir/u.txt")]),
        "the consequence",
    );
    let performed = ok(
        ops::discard_files(git(), &engine(&repo), Confirmed::by_user(consequence), None),
        "the discard",
    );
    let named =
        |locks: &[std::path::PathBuf]| locks.iter().any(|path| path.ends_with("index.lock"));
    assert!(named(&performed.locks().before) && named(&performed.locks().after));
}

// --- A path whose parent is not a real directory (QA item 1) ---

/// `d/a` tracked, then deleted with its directory, and an untracked FILE `d` made where the
/// directory was: `git restore --worktree -- d/a` would unlink `d` to make the directory
/// back, destroying a file no prompt named. The builder refuses the path, and a re-check
/// refuses it when the file appears after the confirmation — with `d` intact both times.
#[test]
fn a_file_where_a_deleted_files_directory_was_is_never_destroyed() {
    let make = || {
        let repo = Repo::new("parent-file");
        repo.write("d/a", b"tracked\n");
        repo.commit("base");
        std::fs::remove_dir_all(repo.path().join("d")).unwrap_or_else(|e| panic!("{e}"));
        repo
    };
    let repo = make();
    repo.write("d", b"an untracked file where the directory was\n");
    assert_eq!(status(&repo), [" D d/a", "?? d"]);
    let outcome = ops::discard_files_consequence(git(), &engine(&repo), &[RepoPath::new("d/a")]);
    assert!(is_refused(&outcome, Refusal::Obstructed), "{outcome:?}");

    let repo = make();
    let consequence = ok(
        ops::discard_files_consequence(git(), &engine(&repo), &[RepoPath::new("d/a")]),
        "the consequence, while nothing is at d",
    );
    repo.write("d", b"made after the confirmation\n");
    let outcome = ops::discard_files(git(), &engine(&repo), Confirmed::by_user(consequence), None);
    assert!(
        matches!(&outcome, Err(Error::ChangedSinceConfirmed { path }) if path == "d/a"),
        "{outcome:?}"
    );
    assert_eq!(
        on_disk(&repo, "d"),
        Some(b"made after the confirmation\n".to_vec())
    );
}

/// `d/a` tracked, and `d` replaced by a symlink to a directory outside the working tree
/// holding an `a` of its own: hashing `d/a` would read a file outside the working tree, and
/// `git restore` would replace the link with a directory. Refused before any prompt, by a
/// discard of files and of lines alike, and the file outside untouched.
#[test]
fn a_symlinked_parent_is_never_followed_out_of_the_working_tree() {
    let repo = Repo::new("parent-link");
    repo.write("d/a", b"tracked\n");
    repo.commit("base");
    let outside = Repo::new("parent-link-outside");
    outside.write("a", b"outside the working tree\n");
    std::fs::remove_dir_all(repo.path().join("d")).unwrap_or_else(|e| panic!("{e}"));
    std::os::unix::fs::symlink(outside.path(), repo.path().join("d"))
        .unwrap_or_else(|e| panic!("{e}"));
    let outcome = ops::discard_files_consequence(git(), &engine(&repo), &[RepoPath::new("d/a")]);
    assert!(is_refused(&outcome, Refusal::Obstructed), "{outcome:?}");
    assert_eq!(
        on_disk(&outside, "a"),
        Some(b"outside the working tree\n".to_vec())
    );
    assert!(
        std::fs::symlink_metadata(repo.path().join("d"))
            .is_ok_and(|metadata| metadata.file_type().is_symlink())
    );
}

/// QA item 5: a discard applies the patch built from the diff the user confirmed, never one
/// rebuilt from a diff handed in later. A diff of the same two blobs can align their lines
/// otherwise (`x` then `y x` added, or `x y` added before `x`), and the same selection then
/// names other lines; here the confirmed selection is the third line, which the other
/// alignment does not even change.
#[test]
fn a_discard_applies_the_patch_it_was_confirmed_with() {
    let repo = Repo::new("confirmed-patch");
    repo.write("file.txt", b"x\n");
    repo.commit("base");
    repo.write("file.txt", b"x\ny\nx\n");
    let drawn = diff_of(&repo, "file.txt", WorkingTreeDiff::Unstaged);
    let text = drawn
        .text()
        .unwrap_or_else(|| panic!("no text: {:?}", drawn.content));
    assert_eq!(
        text.changes(),
        [cairn_model::ChangedRange::new(
            cairn_model::LineSpan::at(1, 0),
            cairn_model::LineSpan::at(1, 2)
        )],
        "git aligned the fixture otherwise, so this decides nothing"
    );
    let mut third = Selection::empty();
    third.select_added(cairn_model::LineNumber::from_index(2));
    let consequence = ok(
        ops::discard_lines_consequence(git(), &engine(&repo), &drawn, third),
        "the consequence",
    );
    ok(
        ops::discard_lines(git(), &engine(&repo), Confirmed::by_user(consequence), None),
        "the discard",
    );
    assert_eq!(on_disk(&repo, "file.txt"), Some(b"x\ny\n".to_vec()));
}

// --- A discard that does not discard everything it was confirmed for (QA items 3, 4) ---

/// QA item 3: `git clean` deletes what it can and exits non-zero on what it cannot — here a
/// file in a directory the user may not write — after `git restore` has already run. The
/// outcome is not a bare failure: it carries the record of what ran, quoting the prompt the
/// user accepted, git's failure, and exactly the confirmed paths still as they were,
/// found by reading each again after the run.
#[test]
fn a_discard_that_fails_part_way_says_what_it_did_and_what_is_left() {
    let repo = modified_and_untracked("partial");
    repo.write("ro/kept.txt", b"cannot be removed\n");
    let paths = [
        RepoPath::new("a.txt"),
        RepoPath::new("dir/u.txt"),
        RepoPath::new("ro/kept.txt"),
    ];
    let consequence = ok(
        ops::discard_files_consequence(git(), &engine(&repo), &paths),
        "the consequence",
    );
    let prompt = consequence.prompt();
    let read_only = repo.path().join("ro");
    std::fs::set_permissions(&read_only, std::fs::Permissions::from_mode(0o555))
        .unwrap_or_else(|e| panic!("{e}"));
    let outcome = ops::discard_files(git(), &engine(&repo), Confirmed::by_user(consequence), None);
    std::fs::set_permissions(&read_only, std::fs::Permissions::from_mode(0o755))
        .unwrap_or_else(|e| panic!("{e}"));
    match outcome {
        Err(Error::DiscardIncomplete {
            performed,
            kept,
            failure,
        }) => {
            assert_eq!(performed.acknowledged(), Some(prompt.as_str()));
            assert_eq!(kept, ["ro/kept.txt"]);
            assert!(
                matches!(failure.as_deref(), Some(Error::GitFailed { arguments, .. }) if arguments.contains("clean")),
                "{failure:?}"
            );
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(on_disk(&repo, "a.txt"), Some(lines(base).into_bytes()));
    assert_eq!(on_disk(&repo, "dir/u.txt"), None);
    assert_eq!(
        on_disk(&repo, "ro/kept.txt"),
        Some(b"cannot be removed\n".to_vec())
    );
}

/// QA item 4 (phase 03), the `IndexSide::Absent` arm: the engine counts any path absent from
/// the index as untracked, so a path git does not list — an ignored file — is offered and
/// reaches `git clean -f`, which leaves it and exits 0; the discard does not claim it, and the
/// outcome names it as kept. The engine trusts its caller here: the window asks only for paths
/// of rows `git status` listed (phase 07's decision; `local_changes_actions`, pinned by
/// `a_discard_names_only_paths_the_lists_drawn_still_list` in `cairn-app`).
#[test]
fn a_file_git_clean_leaves_is_named_as_kept() {
    let repo = Repo::new("ignored");
    repo.write(".gitignore", b"ignored.txt\n");
    repo.commit("base");
    repo.write("ignored.txt", b"ignored\n");
    let consequence = ok(
        ops::discard_files_consequence(git(), &engine(&repo), &[RepoPath::new("ignored.txt")]),
        "the consequence",
    );
    let outcome = ops::discard_files(git(), &engine(&repo), Confirmed::by_user(consequence), None);
    match outcome {
        Err(Error::DiscardIncomplete { kept, failure, .. }) => {
            assert_eq!(kept, ["ignored.txt"]);
            assert!(failure.is_none(), "{failure:?}");
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(on_disk(&repo, "ignored.txt"), Some(b"ignored\n".to_vec()));
}

// --- The user's decisions 5 and 6 of 2026-10-08 ---

/// Decision 5: an intent-to-add file's discard is `git restore`'s — the file left EMPTY, its
/// intent-to-add entry in place — and the prompt says it is emptied, never "modified".
#[test]
fn an_intent_to_add_files_discard_empties_it_and_says_so() {
    let repo = Repo::new("intent-to-add");
    repo.write("tracked.txt", b"t\n");
    repo.commit("base");
    repo.write("new.txt", lines(base).as_bytes());
    repo.git(&["add", "-N", "new.txt"]);
    let consequence = ok(
        ops::discard_files_consequence(git(), &engine(&repo), &[RepoPath::new("new.txt")]),
        "the consequence",
    );
    assert_eq!(
        consequence.prompt(),
        "Do you want to discard the changes in new.txt? 1 new file emptied (20 lines). You \
         can't undo this action."
    );
    ok(
        ops::discard_files(git(), &engine(&repo), Confirmed::by_user(consequence), None),
        "the discard",
    );
    assert_eq!(on_disk(&repo, "new.txt"), Some(Vec::new()));
    assert_eq!(repo.git(&["ls-files", "--", "new.txt"]).trim(), "new.txt");
}

/// Decision 6: a whole file's mode change is named with both modes — alone, and beside its
/// lines — and put back by the discard.
#[test]
fn a_whole_files_mode_change_is_named_and_put_back() {
    for (edited, prompt) in [
        (
            false,
            "Do you want to discard the changes in run.sh? 1 modified (the mode change (100644 \
             to 100755)). You can't undo this action.",
        ),
        (
            true,
            "Do you want to discard the changes in run.sh? 1 modified (2 lines and the mode \
             change (100644 to 100755)). You can't undo this action.",
        ),
    ] {
        let repo = Repo::new("whole-mode");
        repo.write("run.sh", lines(base).as_bytes());
        repo.commit("base");
        if edited {
            repo.write(
                "run.sh",
                lines(|n| if n == 5 { "edited".into() } else { base(n) }).as_bytes(),
            );
        }
        repo.chmod("run.sh", 0o755);
        let consequence = ok(
            ops::discard_files_consequence(git(), &engine(&repo), &[RepoPath::new("run.sh")]),
            "the consequence",
        );
        assert_eq!(consequence.prompt(), prompt);
        ok(
            ops::discard_files(git(), &engine(&repo), Confirmed::by_user(consequence), None),
            "the discard",
        );
        let mode = std::fs::metadata(repo.path().join("run.sh"))
            .map(|metadata| metadata.permissions().mode() & 0o111)
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(mode, 0, "the mode was not put back");
        assert_eq!(on_disk(&repo, "run.sh"), Some(lines(base).into_bytes()));
        assert_eq!(status(&repo), Vec::<String>::new());
    }
}

/// The user's decision 12 (phase 04's QA): a second close leaves a write running with nobody
/// reading its pipes, and git dies of `SIGPIPE` at its next line of output. So every local
/// verb, as the engine passes it, writes nothing on success: `git clean` only with `-q`, which
/// is why it is passed (without it, a line per file), and `git restore --worktree` and `git
/// apply` with nothing to say — on the host's git and the floors. Caught by: `-q` dropped
/// from the clean, or a git that starts talking on success.
#[test]
fn the_destructive_verbs_say_nothing_on_success_so_an_orphan_finishes() {
    let repo = Repo::new("orphan-quiet");
    repo.write("tracked.txt", b"one\n");
    repo.commit("base");
    repo.write("tracked.txt", b"two\n");
    repo.write("u1.txt", b"u\n");
    repo.write("u2.txt", b"u\n");
    let quiet = |args: &[&str], stdin: Option<&[u8]>| {
        let (status, stdout, stderr) = repo.run(args, &[], stdin);
        assert!(status.success(), "git {args:?}: {stderr}");
        assert_eq!((stdout.as_str(), stderr.as_str()), ("", ""), "git {args:?}");
    };
    let (_, loud, _) = repo.run(&["clean", "-n", "--", "u1.txt"], &[], None);
    assert!(
        loud.contains("u1.txt"),
        "without -q, git clean names each file: {loud:?}"
    );
    quiet(
        &[
            "--literal-pathspecs",
            "restore",
            "--worktree",
            "--pathspec-from-file=-",
            "--pathspec-file-nul",
        ],
        Some(b"tracked.txt\0"),
    );
    quiet(
        &[
            "--literal-pathspecs",
            "clean",
            "-f",
            "-q",
            "--",
            "u1.txt",
            "u2.txt",
        ],
        None,
    );
    assert_eq!(on_disk(&repo, "u1.txt"), None);
    repo.write("tracked.txt", b"one\ntwo\n");
    quiet(
        &["--literal-pathspecs", "apply", "--whitespace=nowarn", "-"],
        Some(b"diff --git a/tracked.txt b/tracked.txt\n--- a/tracked.txt\n+++ b/tracked.txt\n@@ -1,2 +1 @@\n one\n-two\n"),
    );
    assert_eq!(on_disk(&repo, "tracked.txt"), Some(b"one\n".to_vec()));
}
