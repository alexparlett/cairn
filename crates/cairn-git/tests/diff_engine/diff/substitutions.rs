//! A commit, a blob and a working-tree file read as git reads them where it reads one object for
//! another: a replace ref (`git replace`, `git replace --graft`), unless `core.useReplaceRefs` is
//! false, and a graft file — each read Cairn shows the user against git's own answer.

use cairn_git::{CancelSignal, ChangesRequest, HistoryRequest, Repository, WorkingTreeDiff};
use cairn_model::{CommitDetails, History, Oid, RowContent};

use super::repositories::Repo;
use super::{ok, parity, some};

/// `main`: `A` — `B` — `C`, each editing `f`, `C` also `g`; `side`: `S` on `A`, editing `f` and
/// adding `h`; and `R`, a commit no ref reaches, on `S`, by another author, editing `f` again —
/// what a replace ref can name in `C`'s place. Answers the repository, `C`, `S` and `R`.
fn history(name: &str) -> (Repo, String, String, String) {
    let repo = Repo::new(name);
    repo.write("f", b"one\n");
    repo.write("g", b"g\n");
    repo.commit("A");
    repo.write("f", b"one\ntwo\n");
    repo.commit("B");
    repo.write("f", b"one\ntwo\nthree\n");
    repo.write("g", b"g, edited\n");
    let c = repo.commit("C\n\nC's body").to_string();
    repo.git(&["checkout", "-q", "-b", "side", "HEAD~2"]);
    repo.write("f", b"side\n");
    repo.write("h", b"h\n");
    let s = repo.commit("S").to_string();
    repo.git(&["checkout", "-q", "-b", "elsewhere"]);
    repo.write("f", b"replaced\nby R\n");
    repo.git(&["add", "-A"]);
    repo.try_git(
        &["commit", "-q", "-m", "R, in C's place\n\nR's body"],
        &[
            ("GIT_AUTHOR_NAME", "R Eplacer"),
            ("GIT_AUTHOR_EMAIL", "r@example.com"),
            ("GIT_AUTHOR_DATE", "1700000000 +0530"),
            ("GIT_COMMITTER_NAME", "R Committer"),
            ("GIT_COMMITTER_EMAIL", "rc@example.com"),
            ("GIT_COMMITTER_DATE", "1700000100 -0800"),
        ],
        None,
    )
    .unwrap_or_else(|e| panic!("committing R: {e}"));
    let r = repo.git(&["rev-parse", "HEAD"]).trim().to_owned();
    repo.git(&["checkout", "-q", "main"]);
    repo.git(&["branch", "-q", "-D", "elsewhere"]);
    (repo, c, s, r)
}

/// One way git reads another object in `C`'s place, or in place of a blob `C` holds.
type Substitute = fn(&Repo, &str, &str, &str);

fn substitutions() -> [(&'static str, Substitute); 6] {
    [
        ("nothing substituted", |_, _, _, _| {}),
        ("git replace --graft", |repo, c, s, _| {
            repo.git(&["replace", "--graft", c, s]);
        }),
        ("git replace", |repo, c, _, r| {
            repo.git(&["replace", c, r]);
        }),
        ("git replace, core.useReplaceRefs false", |repo, c, _, r| {
            repo.git(&["replace", c, r]);
            repo.config("core.useReplaceRefs", "false");
        }),
        ("a graft file", |repo, c, s, _| {
            repo.write(".git/info/grafts", format!("{c} {s}\n").as_bytes());
        }),
        // `f` as `C`'s parent holds it, which the working tree does not: where the index's own
        // entry is the replaced blob, `git diff-tree -p` reads the working-tree file instead of
        // it (`reuse_worktree_file` in git's `diff.c`), which no read can follow.
        ("a blob replaced", |repo, c, _, _| {
            let blob = repo.git(&["rev-parse", &format!("{c}~1:f")]);
            repo.write("other", b"a blob\nread in f's place\n");
            let other = repo.git(&["hash-object", "-w", "other"]);
            repo.remove("other");
            repo.git(&["replace", blob.trim(), other.trim()]);
        }),
    ]
}

/// `id`'s details as `git show -s` prints them, in the details' own shape.
fn shown_details(repo: &Repo, id: &str) -> CommitDetails {
    let shown = repo.git(&[
        "show",
        "-s",
        "--format=%P%x00%an%x00%ae%x00%at%x00%cn%x00%ce%x00%ct%x00%B",
        id,
    ]);
    let fields: Vec<&str> = shown.splitn(8, '\0').collect();
    let field = |at: usize| *some(fields.get(at), "a field of git show's");
    CommitDetails {
        id: ok(Oid::parse(id), "an id"),
        parents: field(0)
            .split_whitespace()
            .map(|parent| ok(Oid::parse(parent), "a parent"))
            .collect(),
        author: cairn_model::Signature {
            name: field(1).to_owned(),
            email: field(2).to_owned(),
            time: cairn_model::Timestamp::new(ok(field(3).parse(), "a date"), 0),
        },
        committer: cairn_model::Signature {
            name: field(4).to_owned(),
            email: field(5).to_owned(),
            time: cairn_model::Timestamp::new(ok(field(6).parse(), "a date"), 0),
        },
        message: field(7).trim_end().to_owned(),
    }
}

/// Cairn's details in the shape [`shown_details`] can hold: offsets dropped (`%at` has none),
/// the message's trailing new lines dropped as `git show`'s terminator makes them ambiguous.
fn comparable(mut details: CommitDetails) -> CommitDetails {
    details.author.time = cairn_model::Timestamp::new(details.author.time.seconds, 0);
    details.committer.time = cairn_model::Timestamp::new(details.committer.time.seconds, 0);
    details.message = details.message.trim_end().to_owned();
    details
}

/// Each commit `HEAD`'s history draws, as `id subject author date`, in id order.
fn cairn_rows(engine: &Repository) -> Vec<String> {
    let page = ok(
        engine.history(&HistoryRequest::from_head(100), &CancelSignal::new()),
        "a page of history",
    );
    let mut history = History::new();
    ok(history.append(page.rows), "holding the page");
    let mut rows: Vec<String> = history
        .rows()
        .map(|row| match row.content() {
            RowContent::Commit(commit) => format!(
                "{} {} {} {}",
                commit.id, commit.summary, commit.author_name, commit.author_time
            ),
            RowContent::Stash(stash) => panic!("a walk from HEAD drew a stash: {stash:?}"),
        })
        .collect();
    rows.sort();
    rows
}

fn git_rows(repo: &Repo) -> Vec<String> {
    let mut rows: Vec<String> = repo
        .git(&["log", "--format=%H %s %an %at", "HEAD"])
        .lines()
        .map(str::to_owned)
        .collect();
    rows.sort();
    rows
}

/// Under each substitution — `C`'s parents replaced (`git replace --graft`), `C` replaced by
/// another commit with its own author, committer, message, tree and parents (`git replace`),
/// the same with `core.useReplaceRefs` false (where git reads `C` itself), `C` grafted by the
/// graft file, and a blob `C` changes replaced by another — every read of `C` Cairn shows is
/// git's: its details against `git show -s`, through the details query and through the changes
/// query; its changed files against `git log --raw`; each text file's lines against `git diff`;
/// the history's rows — their subjects, authors and dates — against `git log`; and Recent
/// Commit Messages against `git log -n 10 --format=%B`. Caught by: a read through gitoxide's own
/// handle, which applies no replace ref when `core.useReplaceRefs` is unset and its own when it
/// is false; the details, a row or a message read from the stored commit; the commit compared
/// with its stored first parent; or a blob read as stored.
#[test]
fn a_substituted_commit_is_described_and_compared_as_git_shows_it() {
    let mut found = Vec::new();
    for (name, substitute) in substitutions() {
        let (repo, c, s, r) = history(&name.replace([' ', ','], "-"));
        substitute(&repo, &c, &s, &r);
        let engine = ok(Repository::discover(repo.path()), "the fixture opens");
        let id = ok(Oid::parse(&c), "an id");

        let theirs = shown_details(&repo, &c);
        let details = comparable(ok(engine.commit_details(&id), "C's details"));
        if details != theirs {
            found.push(format!(
                "{name}: the details are not git show's:\n  git:   {theirs:?}\n  cairn: {details:?}"
            ));
        }
        let changes = ok(
            engine.changes(
                super::git(),
                &ChangesRequest::commit(id),
                &CancelSignal::new(),
            ),
            "C's changes",
        );
        let through_changes = comparable(some(changes.details.clone(), "C's details"));
        if through_changes != theirs {
            found.push(format!(
                "{name}: the changes query's details are not git show's: {through_changes:?}"
            ));
        }
        let (shown, _) = super::changes::shown(&repo, &c);
        let listed = super::changes::cairn_rows(&changes.files, c.len());
        if listed != shown {
            found.push(format!(
                "{name}: the changed files are not git log's:\n  git:   {shown:?}\n  cairn: {listed:?}"
            ));
        }
        let mut session = ok(engine.diff_session(), "a diff session");
        let tally = parity::compare_commit(&repo, &mut session, &c, 3, false);
        if tally.files == 0 {
            found.push(format!("{name}: no text file of C was compared"));
        }
        found.extend(
            tally
                .divergent
                .into_iter()
                .map(|divergence| format!("{name}: {divergence}")),
        );

        let (ours, theirs) = (cairn_rows(&engine), git_rows(&repo));
        if ours != theirs {
            found.push(format!(
                "{name}: the history's rows are not git log's:\n  git:   {theirs:?}\n  cairn: {ours:?}"
            ));
        }
        let recent = ok(
            engine.recent_messages(&CancelSignal::new()),
            "Recent Commit Messages",
        );
        let ours: Vec<String> = recent
            .iter()
            .map(|message| message.trim_end().to_owned())
            .collect();
        let theirs: Vec<String> = repo
            .git(&["log", "-n", "10", "--format=%B%x00", "HEAD"])
            .split('\0')
            .map(|message| message.trim().to_owned())
            .filter(|message| !message.is_empty())
            .collect();
        if ours != theirs {
            found.push(format!(
                "{name}: Recent Commit Messages are not git log's:\n  git:   {theirs:?}\n  cairn: {ours:?}"
            ));
        }
    }
    assert!(found.is_empty(), "{}", found.join("\n"));
}

/// A blob `HEAD` holds, replaced by another, is read as git reads it in a staged and an
/// unstaged diff: `HEAD`'s side of the staged diff, and the index's side of the unstaged one,
/// are the replacement's lines, each diff `git diff`'s. Caught by: a blob read as stored, which
/// draws lines git never diffed (or fails as a disagreement with git's own patch).
#[test]
fn a_replaced_blob_is_diffed_in_the_working_tree_as_git_diffs_it() {
    let repo = Repo::new("replaced-blob-working-tree");
    repo.write("f", b"one\ntwo\nthree\n");
    repo.write("g", b"one\ntwo\nthree\n");
    repo.commit("A");
    let stored = repo.git(&["rev-parse", "HEAD:f"]);
    repo.write("other", b"one\nreplaced\nthree\nfour\n");
    let other = repo.git(&["hash-object", "-w", "other"]);
    repo.remove("other");
    repo.git(&["replace", stored.trim(), other.trim()]);
    // `f` staged as an edit of the stored blob; `g`, the same stored blob, edited unstaged.
    repo.write("f", b"one\ntwo\nthree, staged\n");
    repo.git(&["add", "f"]);
    repo.write("g", b"one\ntwo\nthree, unstaged\n");
    for (path, which) in [
        ("f", WorkingTreeDiff::Staged),
        ("g", WorkingTreeDiff::Unstaged),
    ] {
        let answer = super::working_tree::ask(&repo, path, which);
        assert!(answer.is_some(), "{which:?} {path}: no diff");
        super::working_tree::same_as_git(&repo, path, which, &answer);
    }
}
