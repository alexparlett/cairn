//! staging-and-commit's commit engine against real `git` (R6, R1.4): C13's and C24's engine
//! halves, C14's engine half, C2's amend half and C12's identity case, on the host's git and,
//! through `scripts/git-floor.sh`, on 2.30.9 and 2.32.7. Every oracle is git's own — what
//! `git commit -F <file>` stores, `git diff --cached --name-status`, `git log`, `git status`,
//! `git reflog` — never the code under test.
//!
//! Every commit the engine makes here runs with an environment built for the test: `PATH`, an
//! empty `HOME` and the C locale, so no configuration of the machine's user — a global
//! `commit.gpgSign`, a hooks path — reaches it; the fixture's own configuration names the
//! identity.

use cairn_git::ops::{self, Askpass, CommitWatch, GitBinary, GitEnvironment, Hooks};
use cairn_git::{CancelSignal, CommitRefusal, Error, Repository};
use cairn_model::{
    ChangeStatus, ChangedFile, CommitHooks, Confirmed, Consequence, OperationInProgress,
    Publication, Reflog, RepoPath,
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
    let mut output = |_: &str| {};
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
    let mut output = |_: &str| {};
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
    ops::amend_consequence(&engine(repo), &CancelSignal::new())
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
/// change stays staged and no lock is left; the skip, `--no-verify`, commits past it. And the
/// hooks found are the ones git would run: `.git/hooks`, a `core.hooksPath`, an executable
/// file only. Caught by: a failure that drops git's or the hook's words, a lock left, the skip
/// offered with no hook, or a hook counted that git would not run.
#[test]
fn a_failing_pre_commit_hook_fails_the_commit_with_its_output_and_the_skip_commits() {
    let repo = identified("c13-hook");
    repo.write("file.txt", b"one\n");
    repo.commit("base");
    let hooks = || {
        ok(
            engine(&repo).commit_hooks(git(), &CancelSignal::new()),
            "hooks",
        )
    };
    assert_eq!(
        hooks(),
        CommitHooks::default(),
        "a hook counted that is not there"
    );
    hook(&repo, ".git/hooks", "pre-commit", "exit 0\n", 0o644);
    assert!(!hooks().skippable(), "a hook git would not run was counted");
    hook(
        &repo,
        ".git/hooks",
        "pre-commit",
        "echo 'to stdout'\necho 'lint failed: src/a.rs' >&2\nexit 3\n",
        0o755,
    );
    assert_eq!(
        hooks(),
        CommitHooks {
            pre_commit: true,
            commit_msg: false
        }
    );
    repo.write("file.txt", b"two\n");
    repo.git(&["add", "file.txt"]);

    let cancel = CancelSignal::new();
    let (mut started, mut lines) = (0, Vec::<String>::new());
    let mut running = |_| started += 1;
    let mut output = |line: &str| lines.push(line.to_owned());
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
    assert_eq!(hooks(), CommitHooks::default(), ".git/hooks still counted");
    hook(&repo, "my hooks", "commit-msg", "exit 1\n", 0o755);
    assert_eq!(
        hooks(),
        CommitHooks {
            pre_commit: false,
            commit_msg: true
        }
    );
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

/// C14: the button's text and the prompt are rendered from the engine's `Consequence`:
/// `HEAD`'s short id and subject.
#[test]
fn the_amend_button_and_prompt_name_head() {
    let repo = identified("c14-text");
    repo.write("a.txt", b"a\n");
    let head = repo.commit("the subject");
    let consequence = ok(consequence(&repo), "the consequence");
    let short = head.short();
    assert_eq!(consequence.action(), format!("Amend {}", short.as_str()));
    assert_eq!(
        consequence.prompt(),
        format!(
            "Replaces {} 'the subject'. The old commit stays in Show Lost Commits.",
            short.as_str()
        )
    );
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
    (said, logged_a_move_from(repo, &replaced))
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

/// C24: a merge in progress is reported with git's `MERGE_MSG`; once the conflict is staged
/// (`git add`, which `git status` then reports resolved) the commit is the merge commit, its
/// parents `HEAD` and `MERGE_HEAD`; an amend is refused. Caught by: a merge commit with one
/// parent, or an amend offered mid-merge.
#[test]
fn a_merge_in_progress_commits_the_merge_and_refuses_an_amend() {
    let repo = conflicting("c24-merge");
    let _ = repo.run(&["merge", "-q", "other"], &[], None);
    let merge_head = repo.rev("MERGE_HEAD");
    let head = repo.rev("HEAD");
    let message = std::fs::read_to_string(repo.path().join(".git/MERGE_MSG")).unwrap_or_default();
    assert!(message.starts_with("Merge branch 'other'"), "{message}");
    assert_eq!(
        engine(&repo).operation_in_progress(),
        Some(OperationInProgress::Merge {
            message: Some(message)
        })
    );
    assert!(status_says(&repo).contains("You have unmerged paths"));
    assert!(matches!(
        consequence(&repo),
        Err(Error::CommitRefused {
            why: CommitRefusal::InProgress(OperationInProgress::Merge { .. })
        })
    ));
    repo.write("a.txt", b"resolved\n");
    ok(
        ops::stage_files(git(), &engine(&repo), &[RepoPath::new("a.txt")], None),
        "staging the conflict",
    );
    assert!(status_says(&repo).contains("All conflicts fixed but you are still merging"));
    ok(commit(&repo, "the merge", Hooks::Run), "the merge commit");
    assert_eq!(
        repo.git(&["log", "-1", "--format=%P"]).trim(),
        format!("{head} {merge_head}")
    );
    assert_eq!(engine(&repo).operation_in_progress(), None);
}

/// C24: during a rebase, `git am`, a cherry-pick (one, and a sequence whose stopped pick was
/// resolved and committed) and a revert, the engine reports each as `git status` does, and a
/// commit and an amend are refused before git runs, naming it — `HEAD` and the index as they
/// were. Caught by: gix's reading (the committed sequence reads as nothing), or a commit
/// made mid-rebase.
#[test]
fn a_rebase_am_cherry_pick_or_revert_in_progress_refuses_commit_and_amend() {
    let check = |repo: &Repo, operation: OperationInProgress, says: &str| {
        assert_eq!(
            engine(repo).operation_in_progress(),
            Some(operation.clone())
        );
        assert!(status_says(repo).contains(says), "{}", status_says(repo));
        let head = repo.rev("HEAD");
        let index = std::fs::read(repo.path().join(".git/index")).unwrap_or_default();
        assert!(refused_for(commit(repo, "x", Hooks::Run), &operation));
        assert!(
            matches!(consequence(repo), Err(Error::CommitRefused { why: CommitRefusal::InProgress(found) }) if found == operation)
        );
        assert_eq!(repo.rev("HEAD"), head);
        assert_eq!(
            std::fs::read(repo.path().join(".git/index")).unwrap_or_default(),
            index
        );
    };

    let rebase = conflicting("c24-rebase");
    let _ = rebase.run(&["rebase", "other"], &[], None);
    check(&rebase, OperationInProgress::Rebase, "rebas");

    let pick = conflicting("c24-pick");
    let _ = pick.run(&["cherry-pick", "other"], &[], None);
    check(&pick, OperationInProgress::CherryPick, "cherry-pick");

    let revert = conflicting("c24-revert");
    revert.git(&["checkout", "-q", "other"]);
    revert.write("a.txt", b"later\n");
    revert.commit("later");
    let _ = revert.run(&["revert", "--no-edit", "HEAD^"], &[], None);
    check(&revert, OperationInProgress::Revert, "revert");

    let am = conflicting("c24-am");
    let patch = am.git(&["format-patch", "-1", "--stdout", "other"]);
    let _ = am.run(&["am"], &[], Some(patch.as_bytes()));
    check(&am, OperationInProgress::ApplyingPatches, "am session");

    // A sequence: the first pick conflicts, is resolved and committed by git; the second is
    // still to come, and git still says a cherry-pick is in progress.
    let sequence = conflicting("c24-sequence");
    sequence.git(&["checkout", "-q", "other"]);
    sequence.write("b.txt", b"b\n");
    sequence.commit("second");
    sequence.git(&["checkout", "-q", "main"]);
    let _ = sequence.run(&["cherry-pick", "other~1", "other"], &[], None);
    sequence.write("a.txt", b"resolved\n");
    sequence.git(&["add", "a.txt"]);
    sequence.git(&["-c", "core.editor=true", "commit", "-q", "--no-edit"]);
    assert!(!sequence.path().join(".git/CHERRY_PICK_HEAD").exists());
    check(&sequence, OperationInProgress::CherryPick, "cherry-pick");
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
    let mut output = |_: &str| {};
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
        let outcome = ops::amend_consequence(&repo, &CancelSignal::new());
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
