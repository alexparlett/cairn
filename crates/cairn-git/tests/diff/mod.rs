//! C1, C2, C3, C5 and C6, plus the reporter behind C14.

mod repositories;
mod scratch;

mod bare_discovery;
mod bench;
mod changes;
mod content;
mod expansion;
mod fsmonitor;
mod inputs;
mod ownership;
mod parity;
mod patches;
mod staged_renames;
mod staging;
mod stash;
mod working_tree;
mod write_baseline;
mod write_verbs;

/// `unwrap` and `expect` are denied outside a test function, and a helper shared by several
/// tests is not one. These say the same thing with a message that names what failed, which
/// is what `tests/history.rs` already does.
pub fn ok<T, E: std::fmt::Display>(result: Result<T, E>, what: &str) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("{what}: {error}"),
    }
}

/// The `git` Cairn itself would find on this machine, found once for the whole binary: what
/// the changes query runs (decision E). Its environment is the one the application builds,
/// so it reads the same configuration the engine's gix handle does.
pub fn git() -> &'static cairn_git::ops::GitBinary {
    static GIT: std::sync::OnceLock<cairn_git::ops::GitBinary> = std::sync::OnceLock::new();
    GIT.get_or_init(|| {
        ok(
            cairn_git::ops::GitBinary::discover(&cairn_git::ops::Askpass::new(
                "/nonexistent/cairn-askpass",
                None,
            )),
            "git is found",
        )
    })
}

/// Git `2.<minor>.0`, to compare the git in use against.
pub fn since(minor: u32) -> cairn_git::ops::GitVersion {
    cairn_git::ops::GitVersion {
        major: 2,
        minor,
        patch: 0,
    }
}

pub fn some<T>(value: Option<T>, what: &str) -> T {
    match value {
        Some(value) => value,
        None => panic!("{what}"),
    }
}

/// Every file of `set`, page by page and with no budget, in the change set's order: what
/// Expand All reads of a change set that fits its budget. A file whose outcome is a failure
/// fails the whole answer here, since the tests that call this compare every file with its
/// answer alone; `a_failing_file_is_that_files_outcome_and_the_rest_are_read` holds the
/// per-file outcomes themselves.
pub fn every_file(
    session: &mut cairn_git::DiffSession<'_>,
    request: &cairn_git::ChangesRequest,
    set: &cairn_model::ChangeSet,
    options: &cairn_git::ContentOptions,
    cancel: &impl cairn_git::Cancel,
) -> Result<Vec<cairn_model::FileDiff>, cairn_git::Error> {
    every_file_paged(session, request, set, options, cancel).map(|(diffs, _)| diffs)
}

/// [`every_file`], and how many pages it took.
pub fn every_file_paged(
    session: &mut cairn_git::DiffSession<'_>,
    request: &cairn_git::ChangesRequest,
    set: &cairn_model::ChangeSet,
    options: &cairn_git::ContentOptions,
    cancel: &impl cairn_git::Cancel,
) -> Result<(Vec<cairn_model::FileDiff>, usize), cairn_git::Error> {
    let offered: Vec<usize> = (0..set.files.len()).collect();
    let mut pages = 0;
    let mut from = 0;
    let mut diffs = Vec::with_capacity(set.files.len());
    while from < offered.len() {
        let page = session.page(
            git(),
            request,
            cairn_git::Offered {
                changes: set,
                files: &offered[from..],
            },
            None,
            options,
            cancel,
        )?;
        assert!(page.taken > 0, "a page read nothing of what it was offered");
        pages += 1;
        from += page.taken;
        for (_, outcome) in page.files {
            diffs.push(outcome?);
        }
    }
    Ok((diffs, pages))
}
