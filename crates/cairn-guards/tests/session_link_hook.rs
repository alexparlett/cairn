//! The session-link check in `.githooks/commit-msg`, which `.githooks/pre-push` also
//! runs over every outgoing commit. Run against scratch message files.

use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use cairn_guards::repo_root;

// Split so this file's own lines never read as a link to the hook or a reader.
const HOST: &str = concat!("claude", ".ai/code/", "session");
const TRAILER: &str = concat!("Claude", "-Session");

/// Whether the hook accepts `message`, written to a fresh scratch file.
fn accepts(message: &str) -> bool {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let path: PathBuf = std::env::temp_dir().join(format!(
        "cairn-commit-msg-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, message).unwrap_or_else(|e| panic!("writing {}: {e}", path.display()));
    let status = Command::new(repo_root().join(".githooks/commit-msg"))
        .arg(&path)
        .status()
        .unwrap_or_else(|e| panic!("running .githooks/commit-msg: {e}"));
    let _ = std::fs::remove_file(&path);
    status.success()
}

#[test]
fn commit_messages_never_carry_a_session_link() {
    let refused = [
        format!("docs(docs): a change\n\nWhy it matters.\n\n{TRAILER}: https://{HOST}_01ABC\n"),
        format!("docs(docs): a change\n\nSee https://{HOST}_01ABC for the discussion.\n"),
        format!(
            "docs(docs): a change\n\nWhy.\n\n{}: anything\n",
            TRAILER.to_lowercase()
        ),
        format!("docs(docs): a change\n\nWhy.\n\n  {TRAILER}: indented\n"),
        format!(
            "fix(ui): a change\n\nWhy.\n\nHTTPS://{}_01ABC\n",
            HOST.to_uppercase()
        ),
    ];
    for message in &refused {
        assert!(
            !accepts(message),
            "the hook accepted a session link: {message:?}"
        );
    }

    let accepted = [
        "docs(docs): a change\n\nWhy it matters.\n".to_owned(),
        "docs(docs): a change\n\nWhy.\n\nCo-Authored-By: Claude <noreply@anthropic.com>\n"
            .to_owned(),
        "docs(docs): link the claude.ai docs\n\nhttps://claude.ai/code is the product.\n"
            .to_owned(),
        // git strips comment lines, so a template's help text is not a violation.
        format!("docs(docs): a change\n\nWhy.\n# {TRAILER}: https://{HOST}_01ABC\n"),
    ];
    for message in &accepted {
        assert!(
            accepts(message),
            "the hook refused a clean message: {message:?}"
        );
    }
}

#[test]
fn the_pre_push_floor_runs_the_session_link_check() {
    let pre_push = std::fs::read_to_string(repo_root().join(".githooks/pre-push"))
        .unwrap_or_else(|e| panic!("reading .githooks/pre-push: {e}"));
    let code: Vec<&str> = pre_push
        .lines()
        .map(str::trim)
        .filter(|line| !line.starts_with('#'))
        .collect();
    assert!(
        code.iter()
            .any(|line| line.contains(".githooks/commit-msg")),
        ".githooks/pre-push no longer runs .githooks/commit-msg over outgoing commits, so a \
         commit made with --no-verify could publish a session link"
    );
    assert!(
        code.iter().any(|line| line.contains("git log --format=%B")),
        ".githooks/pre-push no longer reads the outgoing commits' messages"
    );
}
