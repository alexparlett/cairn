//! staging-and-commit's commit engine against real `git` (R6, R1.4, R4.7): C13's, C24's and
//! C33's engine halves, C14's engine half, C32's stripspace and config halves, C2's amend half
//! and C12's identity case, on the host's git and, through `scripts/git-floor.sh`, on 2.30.9
//! and 2.32.7. Every oracle is git's own — what `git commit -F <file>` stores, what `git
//! commit` with an editor leaves, `git diff --cached --name-status`, `git log`, `git status`,
//! `git reflog` — never the code under test.
//!
//! Every commit the engine makes here runs with an environment built for the test: `PATH`, an
//! empty `HOME` and the C locale, so no configuration of the machine's user — a global
//! `commit.gpgSign`, a hooks path — reaches it; the fixture's own configuration names the
//! identity.

use cairn_git::ops::{
    self, AmendAnswer, Askpass, CommitCancel, CommitWatch, GitBinary, GitEnvironment, Hooks,
};
use cairn_git::{CancelSignal, CommitRefusal, Error, Repository};
use cairn_model::{
    ChangeStatus, ChangedFile, Confirmed, Consequence, OperationInProgress, Publication, Reflog,
    RepoPath,
};

use super::repositories::{Repo, empty_home};
use super::{git, ok};

/// The `git` the engine commits with: this process's `PATH`, an empty home, the C locale and
/// `extra`.
fn committer_with(extra: &[(&str, &str)]) -> GitBinary {
    let path = std::env::var_os("PATH");
    let extra: Vec<(String, String)> = extra
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect();
    let environment = GitEnvironment::new(
        move |name| match name {
            "PATH" => path.clone(),
            "HOME" | "XDG_CONFIG_HOME" => Some(empty_home().as_os_str().to_owned()),
            "LC_ALL" => Some("C".into()),
            other => extra
                .iter()
                .find(|(named, _)| named == other)
                .map(|(_, value)| value.into()),
        },
        &Askpass::new("/nonexistent/cairn-askpass", None),
    );
    ok(GitBinary::discover_with(environment), "git is found")
}

fn committer() -> &'static GitBinary {
    static GIT: std::sync::OnceLock<GitBinary> = std::sync::OnceLock::new();
    GIT.get_or_init(|| committer_with(&[]))
}

fn engine(repo: &Repo) -> Repository {
    ok(Repository::discover(repo.path()), "the fixture opens")
}

/// A repository whose own configuration names who commits, as a user's would.
fn identified(name: &str) -> Repo {
    let repo = Repo::new(name);
    repo.config("user.name", "Commit Ter");
    repo.config("user.email", "committer@example.com");
    repo
}

fn commit(repo: &Repo, message: &str, hooks: Hooks) -> Result<ops::Performed, Error> {
    commit_by(committer(), repo, message, hooks)
}

fn commit_by(
    git: &GitBinary,
    repo: &Repo,
    message: &str,
    hooks: Hooks,
) -> Result<ops::Performed, Error> {
    let cancel = CancelSignal::new();
    let mut started = 0;
    let mut running = |_| started += 1;
    let mut output = |_: &cairn_model::ScrubbedLines| {};
    ops::commit(
        git,
        &engine(repo),
        message,
        hooks,
        None,
        CommitWatch {
            cancel: &cancel,
            running: &mut running,
            output: &mut output,
        },
    )
}

fn amend_with(repo: &Repo, confirmed: Confirmed, message: &str) -> Result<ops::Performed, Error> {
    let cancel = CancelSignal::new();
    let mut running = |_| {};
    let mut output = |_: &cairn_model::ScrubbedLines| {};
    ops::amend(
        committer(),
        &engine(repo),
        confirmed,
        message,
        Hooks::Run,
        None,
        CommitWatch {
            cancel: &cancel,
            running: &mut running,
            output: &mut output,
        },
    )
}

fn consequence(repo: &Repo) -> Result<Consequence, Error> {
    ops::amend_consequence(committer(), &engine(repo), &CancelSignal::new())
}

/// Amends at the press (R6.4): run where it is recoverable, its `Consequence` answered where it
/// must be confirmed.
fn amend_at_press(repo: &Repo, message: &str, hooks: Hooks) -> Result<AmendAnswer, Error> {
    let cancel = CancelSignal::new();
    let mut running = |_| {};
    let mut output = |_: &cairn_model::ScrubbedLines| {};
    ops::amend_unconfirmed(
        committer(),
        &engine(repo),
        message,
        hooks,
        None,
        CommitWatch {
            cancel: &cancel,
            running: &mut running,
            output: &mut output,
        },
    )
}

/// The bytes of `HEAD`'s message as stored: everything after the commit's header.
fn stored_message(repo: &Repo) -> Vec<u8> {
    let object = repo.git(&["cat-file", "commit", "HEAD"]);
    match object.split_once("\n\n") {
        Some((_, message)) => message.as_bytes().to_vec(),
        None => Vec::new(),
    }
}

fn hook(repo: &Repo, directory: &str, name: &str, body: &str, mode: u32) {
    let path = format!("{directory}/{name}");
    repo.write(&path, format!("#!/bin/sh\n{body}").as_bytes());
    repo.chmod(&path, mode);
}

// --- C13: the message, byte for byte, as `git commit -F` leaves it ---

/// C13 and the QA brief, and the phase's stopping rule: under every `commit.cleanup` value
/// (and none), with `core.commentChar` unset and `;`, a draft with `#` and `;` lines, CRLF
/// endings, trailing spaces and blank lines, a scissors line and non-ASCII text is stored as
/// the same git stores from `git commit -F <file>`, by commit and by amend. Caught by: a
/// `--cleanup` passed, the message trimmed or re-encoded by Cairn, or a newline added.
#[test]
fn a_message_is_stored_as_git_commit_f_stores_it_under_every_cleanup() {
    let draft = "  Subject é  \r\n\r\n# hash line\r\n; semi line\r\nbody  \n\
                 # ------------------------ >8 ------------------------\nafter\n\n\n\n";
    for cleanup in [
        None,
        Some("default"),
        Some("verbatim"),
        Some("whitespace"),
        Some("strip"),
        Some("scissors"),
    ] {
        for comment in [None, Some(";")] {
            let made = |name: &str| {
                let repo = identified(name);
                if let Some(cleanup) = cleanup {
                    repo.config("commit.cleanup", cleanup);
                }
                if let Some(comment) = comment {
                    repo.config("core.commentChar", comment);
                }
                repo.write("file.txt", b"one\n");
                repo.git(&["add", "file.txt"]);
                repo
            };
            let label = format!("cleanup {cleanup:?}, comment {comment:?}");
            let cairn = made("c13-cleanup-cairn");
            ok(commit(&cairn, draft, Hooks::Run), &label);
            let theirs = made("c13-cleanup-git");
            theirs.write("../draft", draft.as_bytes());
            theirs.git(&["commit", "-q", "-F", "../draft"]);
            assert_eq!(
                String::from_utf8_lossy(&stored_message(&cairn)),
                String::from_utf8_lossy(&stored_message(&theirs)),
                "{label}"
            );

            // The amend's message, the same way.
            cairn.write("file.txt", b"two\n");
            cairn.git(&["add", "file.txt"]);
            let confirmed = Confirmed::by_user(ok(consequence(&cairn), &label));
            ok(amend_with(&cairn, confirmed, draft), &label);
            theirs.write("file.txt", b"two\n");
            theirs.git(&["add", "file.txt"]);
            theirs.git(&["commit", "-q", "--amend", "-F", "../draft"]);
            assert_eq!(
                stored_message(&cairn),
                stored_message(&theirs),
                "amend, {label}"
            );
            let _ = std::fs::remove_file(theirs.path().join("../draft"));
        }
    }
}

/// C13: a non-UTF-8 `i18n.commitEncoding` is refused before git runs, `HEAD` and the index
/// untouched; git's own spellings of UTF-8 commit. Caught by: a Latin-1 setting let through,
/// which stores the UTF-8 draft's bytes as Latin-1.
#[test]
fn a_non_utf8_commit_encoding_is_refused_before_git_runs() {
    let repo = identified("c13-encoding");
    repo.write("file.txt", b"one\n");
    repo.commit("base");
    repo.write("file.txt", b"two\n");
    repo.git(&["add", "file.txt"]);
    let index = std::fs::read(repo.path().join(".git/index")).unwrap_or_default();
    repo.config("i18n.commitEncoding", "ISO-8859-1");
    match commit(&repo, "café", Hooks::Run) {
        Err(Error::CommitRefused {
            why: CommitRefusal::CommitEncoding { encoding },
        }) => assert_eq!(encoding, "ISO-8859-1"),
        other => panic!("a Latin-1 commit encoding was not refused: {other:?}"),
    }
    assert_eq!(repo.git(&["log", "--format=%s"]), "base\n");
    assert_eq!(
        std::fs::read(repo.path().join(".git/index")).unwrap_or_default(),
        index
    );
    let base = Confirmed::by_user(ok(
        {
            repo.config("i18n.commitEncoding", "UTF-8");
            consequence(&repo)
        },
        "the amend's consequence",
    ));
    repo.config("i18n.commitEncoding", "latin1");
    assert!(
        matches!(
            amend_with(&repo, base, "café"),
            Err(Error::CommitRefused {
                why: CommitRefusal::CommitEncoding { .. }
            })
        ),
        "a Latin-1 amend was not refused"
    );
    repo.config("i18n.commitEncoding", "utf8");
    ok(commit(&repo, "café", Hooks::Run), "utf8 commits");
    assert_eq!(repo.git(&["log", "-1", "--format=%s"]), "café\n");
}

/// C13: a failing `pre-commit` hook fails the commit with its output — stdout and stderr
/// alike, handed on as it ran and carried on the failure — nothing is committed, the staged
/// change stays staged and no lock is left; the skip, `--no-verify`, commits past it; and a
/// `commit-msg` hook in a `core.hooksPath` git resolves fails it too, git finding the hook (C4:
/// Cairn keeps no hook model, so the skip is offered on every failure). Caught by: a failure
/// that drops git's or the hook's words, a lock left, or the skip not passing `--no-verify`.
#[test]
fn a_failing_pre_commit_hook_fails_the_commit_with_its_output_and_the_skip_commits() {
    let repo = identified("c13-hook");
    repo.write("file.txt", b"one\n");
    repo.commit("base");
    hook(
        &repo,
        ".git/hooks",
        "pre-commit",
        "echo 'to stdout'\necho 'lint failed: src/a.rs' >&2\nexit 3\n",
        0o755,
    );
    repo.write("file.txt", b"two\n");
    repo.git(&["add", "file.txt"]);

    let cancel = CancelSignal::new();
    let (mut started, mut lines) = (0, Vec::<String>::new());
    let mut running = |_| started += 1;
    let mut output =
        |said: &cairn_model::ScrubbedLines| lines.extend(said.lines().map(str::to_owned));
    let outcome = ops::commit(
        committer(),
        &engine(&repo),
        "subject",
        Hooks::Run,
        None,
        CommitWatch {
            cancel: &cancel,
            running: &mut running,
            output: &mut output,
        },
    );
    match outcome {
        Err(Error::GitFailed {
            stderr,
            present_locks,
            ..
        }) => {
            assert!(stderr.contains("lint failed: src/a.rs"), "{stderr}");
            assert!(stderr.contains("to stdout"), "{stderr}");
            assert_eq!(present_locks, Vec::<std::path::PathBuf>::new());
        }
        other => panic!("the hook's failure was not the commit's: {other:?}"),
    }
    assert_eq!(started, 1);
    assert!(
        lines.iter().any(|line| line == "lint failed: src/a.rs")
            && lines.iter().any(|line| line == "to stdout"),
        "the hook's lines were not handed on: {lines:?}"
    );
    assert_eq!(repo.git(&["log", "--format=%s"]), "base\n");
    assert_eq!(repo.git(&["diff", "--cached", "--name-only"]), "file.txt\n");
    assert!(!repo.path().join(".git/index.lock").exists());

    ok(commit(&repo, "subject", Hooks::Skip), "the skip");
    assert_eq!(repo.git(&["log", "--format=%s"]), "subject\nbase\n");

    // A hooks path git resolves, and a commit-msg hook in it.
    repo.config("core.hooksPath", "my hooks");
    hook(&repo, "my hooks", "commit-msg", "exit 1\n", 0o755);
    repo.write("file.txt", b"three\n");
    repo.git(&["add", "file.txt"]);
    assert!(
        matches!(
            commit(&repo, "refused", Hooks::Run),
            Err(Error::GitFailed { .. })
        ),
        "the commit-msg hook in core.hooksPath did not run"
    );
}

/// C13 and R6.8: with no identity configured, git's own error is the outcome — nothing is
/// committed. Caught by: an identity Cairn made up, or a failure that loses git's words.
#[test]
fn with_no_identity_gits_own_error_is_the_outcome() {
    let repo = Repo::new("c13-no-identity");
    repo.config("user.useConfigOnly", "true");
    repo.write("file.txt", b"one\n");
    repo.git(&["add", "file.txt"]);
    match commit(&repo, "subject", Hooks::Run) {
        Err(Error::GitFailed { stderr, .. }) => {
            assert!(stderr.contains("no email was given"), "{stderr}");
        }
        other => panic!("a commit with no identity was not git's failure: {other:?}"),
    }
    assert!(
        repo.run(&["rev-parse", "--verify", "HEAD"], &[], None)
            .0
            .code()
            != Some(0)
    );
}

/// C12's identity case and R5.2: a commit made with `GIT_AUTHOR_EMAIL` and the committer's
/// variables in Cairn's own environment is by them, over the repository's configuration — a
/// terminal's `git commit` is. Caught by: an identity variable dropped from the inherited
/// roster.
#[test]
fn a_commit_is_by_the_identity_in_cairns_environment() {
    let repo = identified("c12-identity");
    repo.write("file.txt", b"one\n");
    repo.git(&["add", "file.txt"]);
    let git = committer_with(&[
        ("GIT_AUTHOR_NAME", "Env Author"),
        ("GIT_AUTHOR_EMAIL", "env-author@example.com"),
        ("GIT_COMMITTER_EMAIL", "env-committer@example.com"),
    ]);
    ok(commit_by(&git, &repo, "subject", Hooks::Run), "the commit");
    assert_eq!(
        repo.git(&["log", "-1", "--format=%an <%ae> / %cn <%ce>"]),
        "Env Author <env-author@example.com> / Commit Ter <env-committer@example.com>\n"
    );
}

/// C13 and R6.7: the recent messages are `git log -n 10 --format=%B HEAD`'s, newest first,
/// each as written — multi-line, trailing spaces, no trailing newline; and none on an unborn
/// branch. Caught by: subjects for messages, another order, or a count other than ten.
#[test]
fn recent_messages_are_git_logs_last_ten() {
    let repo = identified("c13-recent");
    assert_eq!(
        ok(
            engine(&repo).recent_messages(&CancelSignal::new()),
            "unborn"
        ),
        Vec::<String>::new()
    );
    for n in 0..13 {
        repo.write("file.txt", format!("{n}\n").as_bytes());
        repo.commit(&format!("subject {n}  \n\nbody line {n}\nsecond line"));
    }
    let messages = ok(
        engine(&repo).recent_messages(&CancelSignal::new()),
        "the recent messages",
    );
    let logged: Vec<String> = repo
        .git(&["log", "-n", "10", "-z", "--format=%B", "HEAD"])
        .split('\0')
        .filter(|message| !message.is_empty())
        .map(str::to_owned)
        .collect();
    assert_eq!(logged.len(), 10);
    assert_eq!(cairn_git::RECENT_MESSAGES, 10);
    assert_eq!(messages, logged);
}

// --- R6.3: amend's staged list, and amend unavailable ---

/// The `--name-status` line git prints for one changed file.
fn name_status(file: &ChangedFile) -> String {
    let letter = match file.status {
        ChangeStatus::Added => "A".to_owned(),
        ChangeStatus::Deleted => "D".to_owned(),
        ChangeStatus::Modified => "M".to_owned(),
        ChangeStatus::TypeChanged => "T".to_owned(),
        ChangeStatus::Renamed(similarity) => format!("R{:03}", similarity.percent()),
        ChangeStatus::Copied(similarity) => format!("C{:03}", similarity.percent()),
    };
    match file.status {
        ChangeStatus::Renamed(_) | ChangeStatus::Copied(_) => {
            format!("{letter}\t{}\t{}", file.old_path, file.new_path)
        }
        _ => format!("{letter}\t{}", file.new_path),
    }
}

fn lines(n: usize, tag: &str) -> String {
    (0..n).map(|i| format!("{tag} line {i}\n")).collect()
}

/// C13 and R6.3: amend's staged list is `git diff --cached --name-status HEAD^` — what `HEAD`
/// changed and what is staged on top, a rename paired as the user's `diff.renames` pairs it,
/// an intent-to-add file left out — and a root commit's is against the empty tree. Caught by:
/// the list against `HEAD` (the plain staged list), or renames unpaired.
#[test]
fn amends_staged_list_is_the_index_against_heads_parent() {
    let repo = identified("c13-amend-list");
    repo.write("kept.txt", lines(20, "kept").as_bytes());
    repo.write("moved.txt", lines(20, "moved").as_bytes());
    repo.write("gone.txt", b"gone\n");
    repo.commit("base");
    repo.write("in-head.txt", b"head\n");
    repo.write("kept.txt", lines(21, "kept").as_bytes());
    repo.commit("head");
    repo.git(&["mv", "moved.txt", "renamed.txt"]);
    repo.git(&["rm", "-q", "gone.txt"]);
    repo.write("staged.txt", b"staged\n");
    repo.write("intent.txt", b"intent\n");
    repo.git(&["add", "staged.txt"]);
    repo.git(&["add", "-N", "intent.txt"]);
    let listed: Vec<String> = ok(
        engine(&repo).amend_staged(git(), &CancelSignal::new()),
        "amend's staged list",
    )
    .iter()
    .map(name_status)
    .collect();
    let expected: Vec<String> = repo
        .git(&["diff", "--cached", "--name-status", "HEAD^"])
        .lines()
        .map(str::to_owned)
        .collect();
    assert!(
        expected.iter().any(|line| line.starts_with('R')),
        "{expected:?}"
    );
    assert_eq!(listed, expected);

    let root = identified("c13-amend-root-list");
    root.write("a.txt", b"a\n");
    root.commit("root");
    root.write("b.txt", b"b\n");
    root.git(&["add", "b.txt"]);
    let empty_tree = root.git(&["hash-object", "-t", "tree", "/dev/null"]);
    let listed: Vec<String> = ok(
        engine(&root).amend_staged(git(), &CancelSignal::new()),
        "a root commit's amend list",
    )
    .iter()
    .map(name_status)
    .collect();
    let expected: Vec<String> = root
        .git(&["diff", "--cached", "--name-status", empty_tree.trim()])
        .lines()
        .map(str::to_owned)
        .collect();
    assert_eq!(listed, expected);
    assert_eq!(listed, ["A\ta.txt", "A\tb.txt"]);
}

/// C13 and R6.3: a root commit's amend works, and a file unstaged out of it — `git rm --cached
/// -f` — leaves the amended root without it. Caught by: an amend refused for having no parent.
#[test]
fn a_root_commits_amend_works_and_unstages_with_rm_cached() {
    let repo = identified("c13-root-amend");
    repo.write("a.txt", b"a\n");
    repo.write("b.txt", b"b\n");
    repo.commit("root");
    repo.write("b.txt", b"edited\n");
    repo.git(&["add", "b.txt"]);
    repo.write("b.txt", b"working\n");
    ok(
        ops::unstage_files(
            git(),
            &engine(&repo),
            &[RepoPath::new("b.txt")],
            &ops::UnstageTo::Nothing,
            None,
        ),
        "the unstage out of the root's amend",
    );
    let confirmed = Confirmed::by_user(ok(consequence(&repo), "the root's consequence"));
    ok(
        amend_with(&repo, confirmed, "root, amended"),
        "the root's amend",
    );
    assert_eq!(repo.git(&["log", "--format=%s %P"]), "root, amended \n");
    assert_eq!(repo.git(&["ls-tree", "--name-only", "HEAD"]), "a.txt\n");
    assert_eq!(
        std::fs::read(repo.path().join("b.txt")).unwrap_or_default(),
        b"working\n"
    );
}

/// C13 and R6.3: on an unborn branch there is nothing to amend — refused, and no staged list.
#[test]
fn amend_is_unavailable_on_an_unborn_branch() {
    let repo = identified("c13-unborn");
    repo.write("a.txt", b"a\n");
    repo.git(&["add", "a.txt"]);
    assert!(
        matches!(
            consequence(&repo),
            Err(Error::CommitRefused {
                why: CommitRefusal::NothingToAmend
            })
        ),
        "an unborn branch offered an amend"
    );
    assert!(matches!(
        engine(&repo).amend_staged(git(), &CancelSignal::new()),
        Err(Error::UnbornHead { .. })
    ));
    // And its first commit is made.
    ok(commit(&repo, "first", Hooks::Run), "the first commit");
    assert_eq!(repo.git(&["log", "--format=%s"]), "first\n");
}

// --- C14: whether a remote has HEAD ---

/// A clone of a repository with two commits on `main`, its upstream `origin/main`.
fn cloned(name: &str) -> (Repo, Repo) {
    let origin = identified(&format!("{name}-origin"));
    origin.write("a.txt", b"one\n");
    origin.commit("one");
    origin.write("a.txt", b"two\n");
    origin.commit("two");
    let clone = Repo::new(&format!("{name}-clone"));
    std::fs::remove_dir_all(clone.path().join(".git")).unwrap_or_else(|e| panic!("{e}"));
    clone.git(&[
        "clone",
        "-q",
        origin.path().to_str().unwrap_or_default(),
        ".",
    ]);
    clone.config("user.name", "Commit Ter");
    clone.config("user.email", "committer@example.com");
    (origin, clone)
}

fn published(repo: &Repo) -> Publication {
    match ok(consequence(repo), "the amend's consequence") {
        Consequence::Amend { published, .. } => published,
        other => panic!("{other:?}"),
    }
}

/// C14 and the QA brief: the dialog appears exactly when a remote has `HEAD` — an upstream at
/// `HEAD` or ahead of it has it; one behind it does not, unless another remote branch holds
/// it (phase 05's QA item 1: `Unpublished` means no remote-tracking ref reaches it); with no
/// upstream, another remote
/// branch holding it does, and none at all does not; a fork whose remote branch was deleted
/// (its upstream gone) falls back to every remote; an upstream that is a local branch is no
/// remote; a detached `HEAD` on a remote's commit is published. Caught by: the ahead count
/// read the wrong way round, or a gone or local upstream taken for a remote one.
#[test]
fn the_dialog_is_asked_exactly_when_a_remote_has_head() {
    let (_origin, clone) = cloned("c14");
    let upstream = || Publication::Upstream(cairn_model::RefName::new("refs/remotes/origin/main"));
    assert_eq!(published(&clone), upstream(), "an upstream at HEAD");

    clone.git(&["reset", "-q", "--hard", "HEAD^"]);
    assert_eq!(published(&clone), upstream(), "an upstream ahead of HEAD");

    clone.git(&["reset", "-q", "--hard", "origin/main"]);
    clone.write("b.txt", b"local\n");
    clone.commit("local");
    assert_eq!(
        published(&clone),
        Publication::Unpublished,
        "an upstream behind HEAD"
    );
    // The same, with another remote branch holding HEAD — `git push origin main:feature` —
    // which `git branch -r --contains` names: a remote has it, though the upstream does not.
    clone.git(&["update-ref", "refs/remotes/origin/feature", "HEAD"]);
    assert_eq!(
        clone.git(&["branch", "-r", "--contains", "HEAD"]).trim(),
        "origin/feature"
    );
    assert_eq!(
        published(&clone),
        Publication::SomeRemote,
        "an upstream behind HEAD, another remote branch holding it"
    );
    clone.git(&["update-ref", "-d", "refs/remotes/origin/feature"]);

    // No upstream: a branch of its own, at a commit another remote branch holds, then not.
    clone.git(&["checkout", "-q", "-b", "topic", "origin/main"]);
    clone.git(&["branch", "-q", "--unset-upstream"]);
    assert_eq!(
        published(&clone),
        Publication::SomeRemote,
        "no upstream, held"
    );
    clone.write("c.txt", b"topic\n");
    clone.commit("topic");
    assert_eq!(
        published(&clone),
        Publication::Unpublished,
        "no upstream, not held"
    );

    // A fork whose remote branch was deleted: the upstream is gone.
    clone.git(&["update-ref", "refs/remotes/origin/topic", "HEAD"]);
    clone.git(&["branch", "-q", "--set-upstream-to=origin/topic"]);
    assert_eq!(
        published(&clone),
        Publication::Upstream(cairn_model::RefName::new("refs/remotes/origin/topic"))
    );
    clone.git(&["update-ref", "-d", "refs/remotes/origin/topic"]);
    assert_eq!(
        published(&clone),
        Publication::Unpublished,
        "a gone upstream"
    );
    clone.git(&["update-ref", "refs/remotes/fork/topic", "HEAD"]);
    assert_eq!(
        published(&clone),
        Publication::SomeRemote,
        "a gone upstream, another remote holding HEAD"
    );

    // An upstream that is a local branch is no remote.
    clone.git(&["update-ref", "-d", "refs/remotes/fork/topic"]);
    clone.git(&["branch", "-q", "--set-upstream-to=topic", "main"]);
    clone.git(&["checkout", "-q", "main"]);
    clone.git(&["reset", "-q", "--hard", "topic"]);
    assert_eq!(
        published(&clone),
        Publication::Unpublished,
        "a local upstream"
    );

    // A detached HEAD on a commit a remote has.
    clone.git(&["checkout", "-q", "--detach", "origin/main"]);
    assert_eq!(published(&clone), Publication::SomeRemote, "detached");
}

/// Phase 05's QA item 4: with a commit-graph, whether a remote has `HEAD` is answered from
/// the graph, cut at `HEAD`'s generation, and the answer is the object walk's and git's own —
/// `HEAD` far behind a remote tip, on a side branch no remote reaches, at the tip, and a new
/// commit the graph does not hold (the object walk's again). Caught by: the cut made at the
/// wrong generation (a reached `HEAD` called unpublished), or a commit outside the graph
/// answered from it.
#[test]
fn the_pushed_check_answers_from_a_commit_graph_as_git_does() {
    let (_origin, clone) = cloned("graph");
    for n in 0..30 {
        clone.write("a.txt", format!("{n}\n").as_bytes());
        clone.commit(&format!("ahead {n}"));
    }
    clone.git(&["update-ref", "refs/remotes/origin/far", "HEAD"]);
    clone.git(&["checkout", "-q", "-b", "side", "HEAD~20"]);
    clone.write("side.txt", b"side\n");
    clone.commit("on a side branch");
    clone.git(&["checkout", "-q", "main"]);
    clone.git(&["commit-graph", "write", "--reachable"]);
    assert!(clone.path().join(".git/objects/info/commit-graph").exists());
    let by_git = |head: &str| {
        !clone
            .git(&["for-each-ref", "--contains", head, "refs/remotes/"])
            .trim()
            .is_empty()
    };
    for (what, head, expected) in [
        (
            "far behind a remote tip",
            "origin/far~25",
            Publication::SomeRemote,
        ),
        ("at a remote tip", "origin/far", Publication::SomeRemote),
        ("on a side branch", "side", Publication::Unpublished),
    ] {
        clone.git(&["checkout", "-q", "--detach", head]);
        assert_eq!(published(&clone), expected, "{what}");
        assert_eq!(
            by_git("HEAD"),
            expected == Publication::SomeRemote,
            "git: {what}"
        );
    }
    // HEAD in the graph, and the one remote tip that reaches it outside the graph: the graph
    // cannot answer for a tip it does not hold, so the object walk does (phase 11's QA, TC1).
    clone.git(&["checkout", "-q", "--detach", "side"]);
    clone.write("pushed.txt", b"pushed\n");
    clone.commit("pushed on top of side");
    clone.git(&["update-ref", "refs/remotes/origin/side-pushed", "HEAD"]);
    clone.git(&["checkout", "-q", "--detach", "side"]);
    assert_eq!(
        published(&clone),
        Publication::SomeRemote,
        "HEAD in the graph, its remote tip outside it"
    );
    assert!(by_git("HEAD"));
    clone.git(&["update-ref", "-d", "refs/remotes/origin/side-pushed"]);
    // A commit the graph does not hold: the object walk answers.
    clone.git(&["checkout", "-q", "--detach", "origin/far~3"]);
    clone.write("new.txt", b"new\n");
    clone.commit("not in the graph");
    assert_eq!(published(&clone), Publication::Unpublished);
    clone.write("newer.txt", b"newer\n");
    clone.commit("above it");
    clone.git(&["update-ref", "refs/remotes/origin/new", "HEAD"]);
    clone.git(&["checkout", "-q", "--detach", "HEAD^"]);
    assert_eq!(
        published(&clone),
        Publication::SomeRemote,
        "a remote tip outside the graph"
    );
    assert!(by_git("HEAD"));
}

/// C14's engine half: the `Consequence` names `HEAD`, the remote ref that has it and the
/// reflog fact, and nothing drawn for display — its prompt the fixed sentences of R10.6, its
/// button "Amend". Caught by: a subject back in the value (a display field the re-check would
/// compare), or the prompt drifting from the fixed sentences.
#[test]
fn the_amends_consequence_names_head_and_why_it_is_confirmed() {
    let (_origin, clone) = cloned("c14-text");
    let head = clone.rev("HEAD");
    let short = head.short();
    let read = ok(consequence(&clone), "the consequence");
    assert_eq!(
        read,
        Consequence::Amend {
            commit: head,
            published: Publication::Upstream(cairn_model::RefName::new("refs/remotes/origin/main")),
            reflog: Reflog::Written,
        }
    );
    assert_eq!(read.action(), "Amend");
    assert_eq!(
        read.prompt(),
        format!(
            "{} is already on origin/main. Amending it rewrites history others may have.",
            short.as_str()
        )
    );
    assert!(read.needs_confirming());
}

// --- C14 and R6.4: the amend at the press ---

/// The QA brief, its first case: an amend no remote has and git logs runs at once, unconfirmed
/// — no `Confirmed` is built — and the replaced commit is in the reflog git wrote, so Show Lost
/// Commits draws it as lost. Caught by: the recoverable amend sent to the dialog, or run under
/// a setting git does not log by.
#[test]
fn an_amend_git_logs_and_no_remote_has_runs_at_once() {
    let repo = identified("press-recoverable");
    repo.write("a.txt", b"a\n");
    let replaced = repo.commit("one");
    repo.write("a.txt", b"b\n");
    repo.git(&["add", "a.txt"]);
    let performed = match ok(
        amend_at_press(&repo, "one, amended", Hooks::Run),
        "the press",
    ) {
        AmendAnswer::Amended(performed) => performed,
        AmendAnswer::NeedsConfirming(consequence) => {
            panic!("a recoverable amend asked to be confirmed: {consequence:?}")
        }
    };
    assert_eq!(
        performed.acknowledged(),
        None,
        "a prompt recorded with none shown"
    );
    assert!(performed.invalidated().refs);
    assert_eq!(repo.git(&["log", "--format=%s"]), "one, amended\n");
    assert_ne!(repo.rev("HEAD"), replaced);
    assert!(logged_a_move_from(&repo, &replaced));
    assert_eq!(shown_by_show_lost_commits(&repo, &replaced), Some(true));
}

/// The QA brief, its second case, and C14: the same press on a commit a remote has, and on a
/// repository whose `core.logAllRefUpdates` is false with no log yet — and one whose include
/// sets it only for a linked worktree — runs no git: `HEAD`, the index and the git directory's
/// refs as they were, the `Consequence` answered for the dialog. Caught by: a published or
/// unlogged amend run unconfirmed, or the setting read by another reader than git.
#[test]
fn an_amend_a_remote_has_or_git_logs_nowhere_is_answered_not_run() {
    let unchanged = |repo: &Repo, what: &str| {
        let head = repo.rev("HEAD");
        let index = std::fs::read(repo.path().join(".git/index")).unwrap_or_default();
        let consequence = match ok(amend_at_press(repo, "amended", Hooks::Run), what) {
            AmendAnswer::NeedsConfirming(consequence) => consequence,
            AmendAnswer::Amended(performed) => panic!("{what}: amended unconfirmed: {performed:?}"),
        };
        assert_eq!(repo.rev("HEAD"), head, "{what}");
        assert_eq!(
            std::fs::read(repo.path().join(".git/index")).unwrap_or_default(),
            index,
            "{what}"
        );
        assert_eq!(consequence, ok(self::consequence(repo), what));
        consequence
    };

    let (_origin, clone) = cloned("press-published");
    clone.write("a.txt", b"staged\n");
    clone.git(&["add", "a.txt"]);
    let published = unchanged(&clone, "published");
    assert!(matches!(
        published,
        Consequence::Amend {
            published: Publication::Upstream(_),
            reflog: Reflog::Written,
            ..
        }
    ));

    let unlogged = identified("press-unlogged");
    unlogged.write("a.txt", b"a\n");
    unlogged.commit("one");
    std::fs::remove_dir_all(unlogged.path().join(".git/logs")).unwrap_or_else(|e| panic!("{e}"));
    unlogged.config("core.logAllRefUpdates", "false");
    let read = unchanged(&unlogged, "no reflog");
    assert!(matches!(
        read,
        Consequence::Amend {
            published: Publication::Unpublished,
            reflog: Reflog::NotWritten,
            ..
        }
    ));
    assert!(!logged_a_move_from(&unlogged, &unlogged.rev("HEAD")));

    // Set only through a linked worktree's include: unlogged there, and not in the main one.
    let main = identified("press-include");
    main.write("a.txt", b"a\n");
    main.commit("one");
    let linked = Repo::new("press-include-linked");
    std::fs::remove_dir_all(linked.path()).unwrap_or_else(|e| panic!("{e}"));
    main.git(&[
        "worktree",
        "add",
        "-q",
        "-b",
        "linked",
        linked.path().to_str().unwrap_or_default(),
    ]);
    let git_dir = linked.git(&["rev-parse", "--absolute-git-dir"]);
    let include = main.path().join("../press-include.inc");
    std::fs::write(&include, "[core]\n\tlogAllRefUpdates = false\n")
        .unwrap_or_else(|e| panic!("{e}"));
    main.git(&[
        "config",
        &format!("includeIf.gitdir:{}.path", git_dir.trim()),
        include.to_str().unwrap_or_default(),
    ]);
    std::fs::remove_dir_all(std::path::Path::new(git_dir.trim()).join("logs"))
        .unwrap_or_else(|e| panic!("{e}"));
    std::fs::remove_file(main.path().join(".git/logs/refs/heads/linked"))
        .unwrap_or_else(|e| panic!("{e}"));
    linked.config("user.name", "Commit Ter");
    linked.config("user.email", "committer@example.com");
    let read = unchanged(&linked, "an include's false");
    assert!(matches!(
        read,
        Consequence::Amend {
            reflog: Reflog::NotWritten,
            ..
        }
    ));
    assert!(matches!(
        ok(
            amend_at_press(&main, "main, amended", Hooks::Run),
            "the main worktree"
        ),
        AmendAnswer::Amended(_)
    ));
    let _ = std::fs::remove_file(include);
}

// --- R6.4: the reflog, each arm against what git then writes ---

/// Whether git logged a move away from `replaced` — an entry whose old id is it — in `HEAD`'s
/// reflog or `main`'s: the entry R6.4's `Reflog` says git will write. Read from the log files
/// git names (`git rev-parse --git-path`), since `git reflog` prints each entry's new id only.
fn logged_a_move_from(repo: &Repo, replaced: &cairn_model::Oid) -> bool {
    ["logs/HEAD", "logs/refs/heads/main"].iter().any(|log| {
        let path = repo.git(&["rev-parse", "--git-path", log]);
        let path = repo.path().join(path.trim());
        std::fs::read_to_string(path)
            .unwrap_or_default()
            .lines()
            .any(|entry| entry.split(' ').next() == Some(replaced.to_string().as_str()))
    })
}

/// Amends `repo` through the engine and answers what its consequence said of the reflog, and
/// whether git then logged the replaced commit.
fn reflog_of_an_amend(repo: &Repo) -> (Reflog, bool) {
    let consequence = ok(consequence(repo), "the consequence");
    let Consequence::Amend { commit, reflog, .. } = &consequence else {
        panic!("{consequence:?}");
    };
    let (replaced, said) = (*commit, *reflog);
    ok(
        amend_with(repo, Confirmed::by_user(consequence), "amended"),
        "the amend",
    );
    let logged = logged_a_move_from(repo, &replaced);
    // The amend's entry and Show Lost Commits' walk are one (staging-and-commit R11.1): the
    // replaced commit is drawn when git logged the amend or a ref still reaches it (a branch
    // left on it by a detached amend), and drawn lost exactly when no ref does.
    let reached = repo
        .git(&["rev-list", "--branches", "--remotes", "--tags", "HEAD"])
        .lines()
        .any(|line| line == replaced.to_string());
    let shown = shown_by_show_lost_commits(repo, &replaced);
    assert_eq!(
        shown,
        (logged || reached).then_some(!reached),
        "what Show Lost Commits draws of the replaced commit: drawn, and whether lost"
    );
    (said, logged)
}

/// Whether Show Lost Commits — the walk from every ref and every reflog entry of `HEAD` and
/// each local branch (staging-and-commit R11.1) — draws `commit`, and if so whether as lost,
/// the whole walk paged.
fn shown_by_show_lost_commits(repo: &Repo, commit: &cairn_model::Oid) -> Option<bool> {
    let engine = engine(repo);
    let read = ok(engine.refs(&CancelSignal::new()), "reading the refs");
    let request = cairn_git::HistoryRequest::from_refs(&read.snapshot, 64).with_lost_commits();
    let mut session = ok(engine.history_session(&request), "opening the walk");
    let mut rows = cairn_model::History::new();
    loop {
        let page = ok(session.next_page(64, &CancelSignal::new()), "a page");
        ok(rows.append(page.rows), "holding a page");
        if page.cursor.is_none() {
            break;
        }
    }
    rows.rows()
        .find(|row| row.id() == cairn_model::RowId::Commit(*commit))
        .map(|row| row.is_lost())
}

/// R6.4 as amended, each arm against real git: the default writes the entry; `false` with no
/// log does not; `false` with a log that exists does (git appends); `always` does; a linked
/// worktree of a bare repository does (git does not count it bare); and the same worktree
/// with `false` and no log does not; and under `false`, `HEAD`'s log alone (detached) and the
/// branch's alone each do. Caught by: the setting read as git does not read it, or
/// an existing log ignored.
#[test]
fn whether_the_reflog_is_written_is_what_git_then_does() {
    let made = |name: &str, setting: Option<&str>, keep_logs: bool| {
        let repo = identified(name);
        repo.write("a.txt", b"a\n");
        repo.commit("one");
        if !keep_logs {
            std::fs::remove_dir_all(repo.path().join(".git/logs"))
                .unwrap_or_else(|e| panic!("{e}"));
        }
        if let Some(setting) = setting {
            repo.config("core.logAllRefUpdates", setting);
        }
        repo
    };
    assert_eq!(
        reflog_of_an_amend(&made("reflog-default", None, true)),
        (Reflog::Written, true)
    );
    assert_eq!(
        reflog_of_an_amend(&made("reflog-default-no-logs", None, false)),
        (Reflog::Written, true)
    );
    assert_eq!(
        reflog_of_an_amend(&made("reflog-false", Some("false"), false)),
        (Reflog::NotWritten, false)
    );
    assert_eq!(
        reflog_of_an_amend(&made("reflog-false-kept", Some("false"), true)),
        (Reflog::Written, true)
    );
    assert_eq!(
        reflog_of_an_amend(&made("reflog-always", Some("always"), false)),
        (Reflog::Written, true)
    );

    // One log at a time under `false` (phase 05's QA item 7): a detached `HEAD` with only
    // its own log, and a branch with only the branch's — each enough for git to append.
    let detached = made("reflog-false-head-only", Some("false"), true);
    detached.git(&["checkout", "-q", "--detach"]);
    std::fs::remove_file(detached.path().join(".git/logs/refs/heads/main"))
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        reflog_of_an_amend(&detached),
        (Reflog::Written, true),
        "a detached HEAD's own log"
    );
    let branch = made("reflog-false-branch-only", Some("false"), true);
    std::fs::remove_file(branch.path().join(".git/logs/HEAD")).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        reflog_of_an_amend(&branch),
        (Reflog::Written, true),
        "the branch's own log"
    );
    assert!(
        !branch.path().join(".git/logs/HEAD").exists(),
        "git made HEAD's log under false"
    );

    // A bare repository, amended through a linked worktree.
    let source = identified("reflog-bare-source");
    source.write("a.txt", b"a\n");
    source.commit("one");
    let bare = Repo::new("reflog-bare");
    std::fs::remove_dir_all(bare.path().join(".git")).unwrap_or_else(|e| panic!("{e}"));
    bare.git(&[
        "clone",
        "-q",
        "--bare",
        source.path().to_str().unwrap_or_default(),
        ".",
    ]);
    let worktree = Repo::new("reflog-bare-worktree");
    std::fs::remove_dir_all(worktree.path()).unwrap_or_else(|e| panic!("{e}"));
    bare.git(&[
        "worktree",
        "add",
        "-q",
        worktree.path().to_str().unwrap_or_default(),
        "main",
    ]);
    worktree.config("user.name", "Commit Ter");
    worktree.config("user.email", "committer@example.com");
    assert_eq!(bare.git(&["config", "core.bare"]), "true\n");
    assert_eq!(reflog_of_an_amend(&worktree), (Reflog::Written, true));

    let unlogged = Repo::new("reflog-bare-worktree-false");
    std::fs::remove_dir_all(unlogged.path()).unwrap_or_else(|e| panic!("{e}"));
    bare.git(&[
        "worktree",
        "add",
        "-q",
        "--detach",
        unlogged.path().to_str().unwrap_or_default(),
        "main",
    ]);
    bare.git(&["config", "core.logAllRefUpdates", "false"]);
    let logs = bare.path().join("worktrees");
    for entry in std::fs::read_dir(&logs).into_iter().flatten().flatten() {
        let _ = std::fs::remove_dir_all(entry.path().join("logs"));
    }
    let _ = std::fs::remove_dir_all(bare.path().join("logs"));
    unlogged.config("user.name", "Commit Ter");
    unlogged.config("user.email", "committer@example.com");
    assert_eq!(reflog_of_an_amend(&unlogged), (Reflog::NotWritten, false));
}

// --- C2's amend half: refused, writing nothing, when what was confirmed moved ---

/// C2 and R1.4: `HEAD` moved between the confirmation and the run — another commit made — or
/// a remote came to hold it: the amend refuses, writing nothing. Caught by: an amend that
/// amends whatever `HEAD` is now, under a prompt that named another commit.
#[test]
fn an_amend_refuses_when_head_moved_or_was_published_since_it_was_confirmed() {
    let repo = identified("c2-amend");
    repo.write("a.txt", b"a\n");
    repo.commit("one");
    let confirmed = Confirmed::by_user(ok(consequence(&repo), "the consequence"));
    repo.write("a.txt", b"b\n");
    let moved = repo.commit("two");
    assert!(matches!(
        amend_with(&repo, confirmed, "amended"),
        Err(Error::AmendChangedSinceConfirmed)
    ));
    assert_eq!(repo.rev("HEAD"), moved);
    assert_eq!(repo.git(&["log", "--format=%s"]), "two\none\n");

    let confirmed = Confirmed::by_user(ok(consequence(&repo), "the consequence"));
    repo.git(&["update-ref", "refs/remotes/origin/main", "HEAD"]);
    assert!(matches!(
        amend_with(&repo, confirmed, "amended"),
        Err(Error::AmendChangedSinceConfirmed)
    ));
    assert_eq!(repo.rev("HEAD"), moved);
}

/// C2 and R1.4 for the reflog (phase 05's QA item 9): confirmed while git would log the
/// amend — "the old commit stays in Show Lost Commits" — then the logs removed and the
/// setting turned off, so git no longer would: the amend refuses, writing nothing, rather than
/// replace a commit under a promise that is no longer true. Caught by: a re-check that compares
/// `HEAD` alone.
#[test]
fn an_amend_refuses_when_the_reflog_it_promised_is_gone_since_it_was_confirmed() {
    let repo = identified("c2-amend-reflog");
    repo.write("a.txt", b"a\n");
    let head = repo.commit("one");
    let confirmed = Confirmed::by_user(ok(consequence(&repo), "the consequence"));
    assert!(confirmed.prompt().contains("stays in Show Lost Commits"));
    repo.config("core.logAllRefUpdates", "false");
    std::fs::remove_dir_all(repo.path().join(".git/logs")).unwrap_or_else(|e| panic!("{e}"));
    assert!(matches!(
        amend_with(&repo, confirmed, "amended"),
        Err(Error::AmendChangedSinceConfirmed)
    ));
    assert_eq!(repo.rev("HEAD"), head);
    assert_eq!(repo.git(&["log", "--format=%s"]), "one\n");
}

// --- C24: operations in progress ---

/// The text `git status` says of the operation in progress, in the C locale.
fn status_says(repo: &Repo) -> String {
    repo.git(&["status"])
}

/// Two branches that conflict on `a.txt`: `main` and `other`, `main` checked out.
fn conflicting(name: &str) -> Repo {
    let repo = identified(name);
    repo.write("a.txt", b"base\n");
    repo.commit("base");
    repo.git(&["checkout", "-q", "-b", "other"]);
    repo.write("a.txt", b"other\n");
    repo.commit("other");
    repo.git(&["checkout", "-q", "main"]);
    repo.write("a.txt", b"main\n");
    repo.commit("main");
    repo
}

fn refused_for(outcome: Result<ops::Performed, Error>, operation: &OperationInProgress) -> bool {
    matches!(outcome, Err(Error::CommitRefused { why: CommitRefusal::InProgress(found) }) if &found == operation)
}

/// The message git prepared for the operation in progress, as the engine cleans it.
fn prepared(repo: &Repo) -> Option<String> {
    ok(
        engine(repo).prepared_message(committer(), &CancelSignal::new()),
        "the prepared message",
    )
}

/// C24 and C32: a merge in progress is reported; git's `MERGE_MSG`, cleaned, has no `#
/// Conflicts:` block; once the conflict is staged (`git add`, which `git status` then reports
/// resolved) the commit of that cleaned message is the merge commit, its parents `HEAD` and
/// `MERGE_HEAD`, and its message what `git commit` with an editor leaves; an amend is refused.
/// Caught by: a merge commit with one parent, git's comment lines committed, or an amend
/// offered mid-merge.
#[test]
fn a_merge_in_progress_commits_the_merge_and_refuses_an_amend() {
    let repo = conflicting("c24-merge");
    let _ = repo.run(&["merge", "-q", "other"], &[], None);
    let merge_head = repo.rev("MERGE_HEAD");
    let head = repo.rev("HEAD");
    let message = std::fs::read_to_string(repo.path().join(".git/MERGE_MSG")).unwrap_or_default();
    assert!(message.contains("# Conflicts:"), "{message}");
    assert_eq!(
        engine(&repo).operation_in_progress(),
        Some(OperationInProgress::Merge)
    );
    assert_eq!(prepared(&repo).as_deref(), Some("Merge branch 'other'\n"));
    assert!(status_says(&repo).contains("You have unmerged paths"));
    assert!(matches!(
        consequence(&repo),
        Err(Error::CommitRefused {
            why: CommitRefusal::InProgress(OperationInProgress::Merge)
        })
    ));
    repo.write("a.txt", b"resolved\n");
    ok(
        ops::stage_files(git(), &engine(&repo), &[RepoPath::new("a.txt")], None),
        "staging the conflict",
    );
    assert!(status_says(&repo).contains("All conflicts fixed but you are still merging"));
    let filled = prepared(&repo).unwrap_or_default();
    ok(commit(&repo, &filled, Hooks::Run), "the merge commit");
    assert_eq!(
        repo.git(&["log", "-1", "--format=%P"]).trim(),
        format!("{head} {merge_head}")
    );
    assert_eq!(stored_message(&repo), b"Merge branch 'other'\n");
    assert_eq!(engine(&repo).operation_in_progress(), None);
    assert_eq!(
        prepared(&repo),
        None,
        "a message prepared with no MERGE_MSG"
    );
}

/// A conflicted merge of `other`, its `MERGE_MSG` written under `config` and with `message` as
/// the merge's own message.
fn merging(name: &str, config: &[(&str, &str)], message: &str) -> Repo {
    let repo = conflicting(name);
    for (key, value) in config {
        repo.config(key, value);
    }
    let _ = repo.run(&["merge", "-q", "-m", message, "other"], &[], None);
    assert!(
        repo.path().join(".git/MERGE_HEAD").exists(),
        "{name}: no merge in progress"
    );
    repo.write("a.txt", b"resolved\n");
    repo.git(&["add", "a.txt"]);
    repo
}

/// What `git commit` with an editor that changes nothing stores for the merge in progress —
/// the oracle for what the box shows and commits (C32).
fn committed_by_gits_editor(repo: &Repo) -> Vec<u8> {
    let _ = repo.run(&["commit", "-q"], &[("GIT_EDITOR", "true")], None);
    stored_message(repo)
}

/// C32 and R6.10, the QA brief's last case: a `MERGE_MSG` with `# Conflicts:`, under
/// `core.commentChar` unset and `;`, and under `commit.cleanup=scissors` with its scissors line,
/// is shown cleaned and committed exactly as `git commit` with an editor leaves it — the
/// merge's own `#123` line dropped as a comment where `#` is the comment character and kept
/// where `;` is. Caught by: the read run outside the repository (git would strip `#` whatever
/// the setting), Cairn's own idea of a comment, or the cleaned text committed differently.
#[test]
fn merge_msg_is_cleaned_and_committed_as_gits_editor_leaves_it() {
    let marked = "Merge other\n\n#123 fixes the bug\n;45 also\n";
    for (name, config, message, shown, scissors) in [
        ("default", vec![], marked, "Merge other\n\n;45 also\n", None),
        (
            "semicolon",
            vec![("core.commentChar", ";")],
            marked,
            "Merge other\n\n#123 fixes the bug\n",
            None,
        ),
        (
            "scissors",
            vec![("commit.cleanup", "scissors")],
            "Merge other\n",
            "Merge other\n",
            Some("# ------------------------ >8"),
        ),
        (
            "scissors-semicolon",
            vec![("commit.cleanup", "scissors"), ("core.commentChar", ";")],
            "Merge other\n",
            "Merge other\n",
            Some("; ------------------------ >8"),
        ),
    ] {
        let cairn = merging(&format!("c32-cairn-{name}"), &config, message);
        let written =
            std::fs::read_to_string(cairn.path().join(".git/MERGE_MSG")).unwrap_or_default();
        assert!(written.contains("Conflicts:"), "{name}: {written}");
        if let Some(line) = scissors {
            assert!(
                written.contains(line),
                "{name}: no scissors line: {written}"
            );
        }
        let filled = prepared(&cairn).unwrap_or_default();
        assert_eq!(filled, shown, "{name}");
        ok(commit(&cairn, &filled, Hooks::Run), name);
        assert_eq!(
            stored_message(&cairn),
            shown.as_bytes(),
            "{name}: committed"
        );
        let editor = merging(&format!("c32-git-{name}"), &config, message);
        assert_eq!(
            committed_by_gits_editor(&editor),
            shown.as_bytes(),
            "{name}: shown is not what git's editor leaves"
        );
    }
}

/// R6.10 against git's editor where `git stripspace --strip-comments` — C32's read, whatever
/// `commit.cleanup` says — is not it, each divergence pinned so a change in either is seen:
/// under `scissors` the editor keeps the merge's own `#123` line above the scissors line,
/// which the strip drops; under `whitespace` the editor keeps `# Conflicts:`; and under
/// `core.commentChar=auto` git's commit picks another comment character because a line starts
/// with `#`, keeping `# Conflicts:`, where `stripspace` reads `auto` as `#`. Caught by: the
/// read changed to follow `commit.cleanup` without the PRD saying so, or git changing either.
#[test]
fn where_gits_editor_is_not_the_strip_of_comments_it_is_pinned() {
    let message = "Merge other\n\n#123 fixes the bug\n;45 also\n";
    for (name, config, editor_keeps) in [
        (
            "scissors",
            vec![("commit.cleanup", "scissors")],
            "#123 fixes the bug",
        ),
        (
            "whitespace",
            vec![("commit.cleanup", "whitespace")],
            "# Conflicts:",
        ),
        ("auto", vec![("core.commentChar", "auto")], "# Conflicts:"),
    ] {
        let cairn = merging(&format!("c32-diverge-cairn-{name}"), &config, message);
        let filled = prepared(&cairn).unwrap_or_default();
        assert_eq!(filled, "Merge other\n\n;45 also\n", "{name}");
        let editor = merging(&format!("c32-diverge-git-{name}"), &config, message);
        let left = String::from_utf8_lossy(&committed_by_gits_editor(&editor)).into_owned();
        assert!(
            left.contains(editor_keeps),
            "{name}: git's editor left {left:?}"
        );
    }
}

/// C33 and C24: a single cherry-pick in progress is reported naming the picked commit, its
/// `MERGE_MSG` cleaned; Commit concludes it as `git commit -F` does — the picked commit's author
/// kept, `CHERRY_PICK_HEAD` gone, one parent; and an amend is refused. Caught by: a single pick
/// refused, the author made the committer, or the marker left for git to trip on.
#[test]
fn a_single_cherry_pick_is_concluded_by_commit_keeping_its_author() {
    let made = |name: &str| {
        let repo = conflicting(name);
        repo.git(&["checkout", "-q", "other"]);
        repo.write("a.txt", b"picked\n");
        repo.try_git(
            &["commit", "-q", "-am", "the picked change"],
            &[
                ("GIT_AUTHOR_NAME", "Pick Author"),
                ("GIT_AUTHOR_EMAIL", "pick@example.com"),
            ],
            None,
        )
        .unwrap_or_else(|e| panic!("{e}"));
        let picked = repo.rev("HEAD");
        repo.git(&["checkout", "-q", "main"]);
        let _ = repo.run(&["cherry-pick", &picked.to_string()], &[], None);
        assert!(repo.path().join(".git/CHERRY_PICK_HEAD").exists());
        assert!(
            !repo.path().join(".git/sequencer").exists(),
            "git kept a sequence for one pick"
        );
        repo.write("a.txt", b"resolved\n");
        repo.git(&["add", "a.txt"]);
        (repo, picked)
    };
    let (cairn, picked) = made("c33-pick-cairn");
    let head = cairn.rev("HEAD");
    assert_eq!(
        engine(&cairn).operation_in_progress(),
        Some(OperationInProgress::CherryPick {
            picked: Some(picked)
        })
    );
    assert!(status_says(&cairn).contains("cherry-pick"));
    let filled = prepared(&cairn).unwrap_or_default();
    assert_eq!(filled, "the picked change\n");
    assert!(matches!(
        consequence(&cairn),
        Err(Error::CommitRefused {
            why: CommitRefusal::InProgress(OperationInProgress::CherryPick { .. })
        })
    ));
    ok(commit(&cairn, &filled, Hooks::Run), "the pick concluded");
    let (theirs, _) = made("c33-pick-git");
    theirs.write("../pick-message", filled.as_bytes());
    theirs.git(&["commit", "-q", "-F", "../pick-message"]);
    let _ = std::fs::remove_file(theirs.path().join("../pick-message"));
    // The committer is each one's own — the engine's configured identity, the oracle's
    // environment — and the author and the message git's alike.
    let shown = |repo: &Repo| repo.git(&["log", "-1", "--format=%an <%ae>|%B"]);
    assert_eq!(shown(&cairn), shown(&theirs));
    assert!(
        shown(&cairn).starts_with("Pick Author <pick@example.com>|"),
        "{}",
        shown(&cairn)
    );
    assert_eq!(cairn.git(&["log", "-1", "--format=%cn"]), "Commit Ter\n");
    assert_eq!(
        cairn.git(&["log", "-1", "--format=%P"]).trim(),
        head.to_string()
    );
    assert!(!cairn.path().join(".git/CHERRY_PICK_HEAD").exists());
    assert_eq!(engine(&cairn).operation_in_progress(), None);
}

/// C33: a single revert in progress is reported naming the reverted commit; Commit concludes
/// it as `git commit -F` does — `REVERT_HEAD` gone, the commit by the committer. Caught by: a
/// single revert refused, or its marker left.
#[test]
fn a_single_revert_is_concluded_by_commit() {
    let made = |name: &str| {
        let repo = conflicting(name);
        repo.write("a.txt", b"later\n");
        repo.commit("later");
        let reverted = repo.rev("HEAD^");
        let _ = repo.run(&["revert", "--no-edit", "HEAD^"], &[], None);
        assert!(repo.path().join(".git/REVERT_HEAD").exists());
        repo.write("a.txt", b"resolved\n");
        repo.git(&["add", "a.txt"]);
        (repo, reverted)
    };
    let (cairn, reverted) = made("c33-revert-cairn");
    assert_eq!(
        engine(&cairn).operation_in_progress(),
        Some(OperationInProgress::Revert {
            reverted: Some(reverted)
        })
    );
    let filled = prepared(&cairn).unwrap_or_default();
    assert!(filled.starts_with("Revert \"main\""), "{filled}");
    assert!(!filled.contains("Conflicts"), "{filled}");
    ok(commit(&cairn, &filled, Hooks::Run), "the revert concluded");
    let (theirs, _) = made("c33-revert-git");
    theirs.write("../revert-message", filled.as_bytes());
    theirs.git(&["commit", "-q", "-F", "../revert-message"]);
    let _ = std::fs::remove_file(theirs.path().join("../revert-message"));
    let message = |repo: &Repo| repo.git(&["log", "-1", "--format=%B"]);
    assert_eq!(message(&cairn), message(&theirs));
    // A revert is the committer's own, in each: no author carried from the reverted commit.
    let identity = |repo: &Repo| repo.git(&["log", "-1", "--format=%an <%ae>|%cn <%ce>"]);
    assert_eq!(
        identity(&cairn),
        "Commit Ter <committer@example.com>|Commit Ter <committer@example.com>\n"
    );
    assert_eq!(
        identity(&theirs),
        "A U Thor <author@example.com>|C O Mitter <committer@example.com>\n"
    );
    assert!(!cairn.path().join(".git/REVERT_HEAD").exists());
    assert_eq!(engine(&cairn).operation_in_progress(), None);
}

/// C24 and C33: during a rebase, `git am`, and a sequence of cherry-picks — stopped on its
/// first pick, and with that pick resolved and committed — or of reverts, the engine reports
/// each as `git status` does, and a commit and an amend are refused before git runs, naming it
/// and git's command to continue or abort it — `HEAD` and the index as they were. Caught by:
/// gix's reading (the committed sequence reads as nothing), a sequence's stopped pick concluded
/// as a single one, or a commit made mid-rebase.
#[test]
fn a_rebase_am_or_sequence_in_progress_refuses_commit_and_amend() {
    let check = |repo: &Repo, operation: OperationInProgress, says: &str, command: &str| {
        assert_eq!(engine(repo).operation_in_progress(), Some(operation));
        assert!(status_says(repo).contains(says), "{}", status_says(repo));
        let head = repo.rev("HEAD");
        let index = std::fs::read(repo.path().join(".git/index")).unwrap_or_default();
        let refused = commit(repo, "x", Hooks::Run);
        let text = refused
            .as_ref()
            .err()
            .map(ToString::to_string)
            .unwrap_or_default();
        assert!(refused_for(refused, &operation), "{operation:?}");
        assert!(
            text.contains(&format!("{command} --continue or --abort")),
            "{text}"
        );
        assert!(
            matches!(consequence(repo), Err(Error::CommitRefused { why: CommitRefusal::InProgress(found) }) if found == operation)
        );
        assert!(
            matches!(amend_at_press(repo, "x", Hooks::Run), Err(Error::CommitRefused { why: CommitRefusal::InProgress(found) }) if found == operation)
        );
        assert_eq!(repo.rev("HEAD"), head);
        assert_eq!(
            std::fs::read(repo.path().join(".git/index")).unwrap_or_default(),
            index
        );
    };

    let rebase = conflicting("c24-rebase");
    let _ = rebase.run(&["rebase", "other"], &[], None);
    check(&rebase, OperationInProgress::Rebase, "rebas", "git rebase");

    let am = conflicting("c24-am");
    let patch = am.git(&["format-patch", "-1", "--stdout", "other"]);
    let _ = am.run(&["am"], &[], Some(patch.as_bytes()));
    check(
        &am,
        OperationInProgress::ApplyingPatches,
        "am session",
        "git am",
    );

    // A sequence: the first pick conflicts; then it is resolved and committed by git, the
    // second still to come, and git still says a cherry-pick is in progress.
    let sequence = conflicting("c24-sequence");
    sequence.git(&["checkout", "-q", "other"]);
    sequence.write("b.txt", b"b\n");
    sequence.commit("second");
    sequence.git(&["checkout", "-q", "main"]);
    let _ = sequence.run(&["cherry-pick", "other~1", "other"], &[], None);
    assert!(sequence.path().join(".git/CHERRY_PICK_HEAD").exists());
    check(
        &sequence,
        OperationInProgress::CherryPickSequence,
        "cherry-pick",
        "git cherry-pick",
    );
    sequence.write("a.txt", b"resolved\n");
    sequence.git(&["add", "a.txt"]);
    sequence.git(&["-c", "core.editor=true", "commit", "-q", "--no-edit"]);
    assert!(!sequence.path().join(".git/CHERRY_PICK_HEAD").exists());
    check(
        &sequence,
        OperationInProgress::CherryPickSequence,
        "cherry-pick",
        "git cherry-pick",
    );

    let reverts = conflicting("c24-revert-sequence");
    reverts.git(&["checkout", "-q", "other"]);
    reverts.write("a.txt", b"later\n");
    reverts.commit("later");
    reverts.write("b.txt", b"b\n");
    reverts.commit("latest");
    let _ = reverts.run(&["revert", "--no-edit", "HEAD~2", "HEAD~1"], &[], None);
    if reverts.path().join(".git/sequencer").exists() {
        check(
            &reverts,
            OperationInProgress::RevertSequence,
            "revert",
            "git revert",
        );
    } else {
        panic!("git left no revert sequence: {}", status_says(&reverts));
    }
}

/// C8 and R6.9: on a detached `HEAD` a commit is made — on no branch, refused by nothing, no
/// operation reported — and the refs say `HEAD` is detached. Caught by: a detached `HEAD`
/// refused, or reported as an operation in progress.
#[test]
fn a_commit_on_a_detached_head_is_made() {
    let repo = identified("c8-detached");
    repo.write("a.txt", b"a\n");
    let base = repo.commit("base");
    repo.git(&["checkout", "-q", "--detach"]);
    assert_eq!(engine(&repo).operation_in_progress(), None);
    let refs = ok(engine(&repo).refs(&CancelSignal::new()), "the refs");
    assert_eq!(refs.snapshot.head, cairn_model::HeadState::Detached(base));
    repo.write("a.txt", b"b\n");
    repo.git(&["add", "a.txt"]);
    ok(
        commit(&repo, "on no branch", Hooks::Run),
        "the detached commit",
    );
    assert_eq!(
        repo.git(&["log", "-1", "--format=%s %P"]).trim(),
        format!("on no branch {base}")
    );
    assert_eq!(repo.rev("main"), base, "a branch moved");
}

/// R1.6: an amend's record quotes the prompt the user accepted, and the commit's declares the
/// refs, the index and the objects invalid. Caught by: an amend that loses its prompt, or a
/// commit that leaves the history unread.
#[test]
fn an_amend_records_its_prompt_and_a_commit_invalidates_what_it_moves() {
    let repo = identified("r16-amend");
    repo.write("a.txt", b"a\n");
    repo.git(&["add", "a.txt"]);
    let committed = ok(commit(&repo, "one", Hooks::Run), "the commit");
    assert_eq!(committed.acknowledged(), None);
    let invalidated = committed.invalidated();
    assert!(invalidated.refs && invalidated.index && invalidated.objects);
    let consequence = ok(consequence(&repo), "the consequence");
    let prompt = consequence.prompt();
    let amended = ok(
        amend_with(&repo, Confirmed::by_user(consequence), "one, amended"),
        "the amend",
    );
    assert_eq!(amended.acknowledged(), Some(prompt.as_str()));
    assert!(amended.invalidated().refs);
    assert_eq!(repo.git(&["log", "--format=%s"]), "one, amended\n");
}

/// The cancel before git runs: polled through the amend's checks, it writes nothing. Caught
/// by: a cancel that only works once git is running.
#[test]
fn a_commit_cancelled_before_git_runs_writes_nothing() {
    let repo = identified("cancel-before");
    repo.write("a.txt", b"a\n");
    repo.git(&["add", "a.txt"]);
    let cancel = CancelSignal::new();
    cancel.cancel();
    let mut started = 0;
    let mut running = |_| started += 1;
    let mut output = |_: &cairn_model::ScrubbedLines| {};
    let outcome = ops::commit(
        committer(),
        &engine(&repo),
        "x",
        Hooks::Run,
        None,
        CommitWatch {
            cancel: &cancel,
            running: &mut running,
            output: &mut output,
        },
    );
    assert!(matches!(outcome, Err(Error::CommitCancelledBeforeRunning)));
    assert_eq!(started, 0);
    assert!(
        repo.run(&["rev-parse", "--verify", "HEAD"], &[], None)
            .0
            .code()
            != Some(0)
    );
}

// --- R4.7 and phase 12's QA item #10: made is read from HEAD ---

/// Commits (or, with `amend`, amends at the press) `message` with what is staged, cancelling
/// from another thread as soon as `marker` exists — a hook writes it, then sleeps — and hands
/// back the outcome. A hook refused as "Text file busy", while another test's fork still holds
/// it open, is run again.
fn cancelled_at(repo: &Repo, marker: &std::path::Path, amend: bool) -> Result<bool, Error> {
    use std::time::{Duration, Instant};
    for _ in 0..20 {
        let _ = std::fs::remove_file(marker);
        let cancel = CancelSignal::new();
        let mut waiter = None;
        let watched = marker.to_owned();
        let mut running = |handle: CommitCancel| {
            let watched = watched.clone();
            waiter = Some(std::thread::spawn(move || {
                let deadline = Instant::now() + Duration::from_secs(20);
                while !watched.exists() && Instant::now() < deadline {
                    std::thread::sleep(Duration::from_millis(5));
                }
                handle.cancel();
            }));
        };
        let mut output = |_: &cairn_model::ScrubbedLines| {};
        let watch = CommitWatch {
            cancel: &cancel,
            running: &mut running,
            output: &mut output,
        };
        let outcome = if amend {
            ops::amend_unconfirmed(
                committer(),
                &engine(repo),
                "amended",
                Hooks::Run,
                None,
                watch,
            )
            .map(|answer| matches!(answer, AmendAnswer::Amended(_)))
        } else {
            ops::commit(
                committer(),
                &engine(repo),
                "committed",
                Hooks::Run,
                None,
                watch,
            )
            .map(|_| true)
        };
        if let Some(waiter) = waiter {
            let _ = std::fs::write(marker, b"");
            let _ = waiter.join();
        }
        match &outcome {
            Err(Error::GitFailed { stderr, .. }) if stderr.contains("Text file busy") => {
                std::thread::sleep(Duration::from_millis(20));
            }
            _ => return outcome,
        }
    }
    panic!("the hook stayed busy");
}

/// R4.7: a commit or an amend cancelled while its `pre-commit` hook runs was not made — `HEAD`
/// read after the reap says so — and is the cancel it is; one cancelled while its
/// `post-commit` hook runs, after git made it, is reported made. Caught by: a cancel reported
/// as "may have taken effect" whatever `HEAD` says, or a made commit reported cancelled, which
/// would keep a draft already committed.
#[test]
fn a_cancelled_commit_or_amend_is_made_exactly_when_head_says_so() {
    for amend in [false, true] {
        for (when, made) in [("pre-commit", false), ("post-commit", true)] {
            let label = format!(
                "{} cancelled in {when}",
                if amend { "amend" } else { "commit" }
            );
            let repo = identified(&format!(
                "r47-{}-{when}",
                if amend { "amend" } else { "commit" }
            ));
            repo.write("a.txt", b"a\n");
            repo.commit("base");
            repo.write("a.txt", b"b\n");
            repo.git(&["add", "a.txt"]);
            let marker = repo.path().join(".git/hook-ran");
            hook(
                &repo,
                ".git/hooks",
                when,
                &format!("touch '{}'\nsleep 30\n", marker.display()),
                0o755,
            );
            let before = repo.rev("HEAD");
            let outcome = cancelled_at(&repo, &marker, amend);
            if made {
                assert!(matches!(outcome, Ok(true)), "{label}: {outcome:?}");
                assert_ne!(repo.rev("HEAD"), before, "{label}");
            } else {
                assert!(
                    matches!(outcome, Err(Error::GitCancelled { .. })),
                    "{label}: {outcome:?}"
                );
                assert_eq!(repo.rev("HEAD"), before, "{label}");
            }
        }
    }
}

/// Phase 12's QA item #10, beside "HEAD moved in between": git exits 0, but `HEAD` is not the
/// commit it made — a `post-commit` hook moved it back — so the commit and the amend are
/// reported unconfirmed, never made. Caught by: success read from git's exit status alone.
#[test]
fn a_commit_git_says_it_made_with_head_elsewhere_is_unconfirmed() {
    let repo = identified("r10-moved");
    repo.write("a.txt", b"a\n");
    repo.commit("one");
    repo.write("a.txt", b"b\n");
    repo.commit("two");
    hook(
        &repo,
        ".git/hooks",
        "post-commit",
        "git reset -q --soft HEAD^\n",
        0o755,
    );
    repo.write("a.txt", b"c\n");
    repo.git(&["add", "a.txt"]);
    assert!(
        matches!(
            commit(&repo, "three", Hooks::Run),
            Err(Error::CommitUnconfirmed { verb: "committed" })
        ),
        "a commit git made and something undid was reported made"
    );
    assert_eq!(repo.git(&["log", "--format=%s"]), "two\none\n");
    let confirmed = Confirmed::by_user(ok(consequence(&repo), "the consequence"));
    assert!(
        matches!(
            amend_with(&repo, confirmed, "two, amended"),
            Err(Error::CommitUnconfirmed { verb: "amended" })
        ),
        "a confirmed amend git made and something undid was reported made"
    );
    assert_eq!(repo.git(&["log", "--format=%s"]), "one\n");
    repo.try_git(
        &[
            "-c",
            "core.hooksPath=/nonexistent",
            "commit",
            "-q",
            "-m",
            "two again",
        ],
        &[],
        None,
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert!(
        matches!(
            amend_at_press(&repo, "two, amended", Hooks::Run),
            Err(Error::CommitUnconfirmed { verb: "amended" })
        ),
        "an amend git made and something undid was reported made"
    );
    // Without the hook, each is made and confirmed.
    std::fs::remove_file(repo.path().join(".git/hooks/post-commit"))
        .unwrap_or_else(|e| panic!("{e}"));
    repo.git(&["add", "a.txt"]);
    ok(commit(&repo, "three", Hooks::Run), "the commit");
    assert!(matches!(
        ok(
            amend_at_press(&repo, "three, amended", Hooks::Run),
            "the amend"
        ),
        AmendAnswer::Amended(_)
    ));
    assert_eq!(repo.git(&["log", "--format=%s"]), "three, amended\none\n");
}

/// An amend that makes the very commit it replaces — the same tree, message, parents, author
/// and committer second, so the same object — is made: git exited 0 and `HEAD`'s parents are
/// the replaced commit's. Amended again until one lands in the second of the last. Caught by:
/// a made amend reported unconfirmed for leaving `HEAD` where it was.
#[test]
fn an_amend_that_makes_the_same_commit_is_made() {
    let repo = identified("r10-same");
    repo.write("a.txt", b"a\n");
    repo.git(&["add", "a.txt"]);
    ok(commit(&repo, "same\n", Hooks::Run), "the commit");
    for _ in 0..50 {
        let before = repo.rev("HEAD");
        let answer = amend_at_press(&repo, "same\n", Hooks::Run);
        assert!(matches!(answer, Ok(AmendAnswer::Amended(_))), "{answer:?}");
        if repo.rev("HEAD") == before {
            return;
        }
    }
    panic!("no amend made the commit it replaced within fifty tries");
}

/// The pushed check's cost where history is long (the phase's stopping rule): an ignored
/// reporter that times [`ops::amend_consequence`] on the repository `CAIRN_BENCH_REPO` names,
/// in whatever state its `HEAD` is, and prints what it found. It only reads.
///
/// ```text
/// CAIRN_BENCH_REPO=/tmp/rust-clone cargo test -p cairn-git --release --test diff_engine \
///   -- --ignored --nocapture the_pushed_check
/// ```
#[test]
#[ignore = "a reporter over a large repository CAIRN_BENCH_REPO names"]
fn the_pushed_check_on_a_large_repository() {
    let Some(path) = std::env::var_os("CAIRN_BENCH_REPO") else {
        eprintln!("SKIPPED the pushed check: CAIRN_BENCH_REPO is not set");
        return;
    };
    let repo = ok(Repository::discover(&path), "the bench repository opens");
    for round in 0..3 {
        let started = std::time::Instant::now();
        let outcome = ops::amend_consequence(git(), &repo, &CancelSignal::new());
        let elapsed = started.elapsed();
        match outcome {
            Ok(Consequence::Amend {
                commit, published, ..
            }) => eprintln!(
                "round {round}: {} {published:?} in {elapsed:?}",
                commit.short().as_str()
            ),
            other => eprintln!("round {round}: {other:?} in {elapsed:?}"),
        }
    }
}
