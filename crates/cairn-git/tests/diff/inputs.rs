//! `DiffInputs` and `StagedInputs`: what a commit's diff reads outside its commits, named
//! where git reads it, and the index's part of it compared by what it holds.

use std::path::{Path, PathBuf};

use cairn_git::Repository;

use super::ok;
use super::repositories::Repo;

fn opened(repo: &Repo) -> Repository {
    ok(Repository::discover(repo.path()), "the fixture opens")
}

/// `path` with its longest existing ancestor resolved, so a file that does not exist yet
/// compares equal however the temporary directory is spelled.
fn canonical(path: &Path) -> PathBuf {
    let mut existing = path;
    let mut rest = Vec::new();
    while existing.canonicalize().is_err() {
        let (Some(parent), Some(name)) = (existing.parent(), existing.file_name()) else {
            return path.to_owned();
        };
        rest.push(name);
        existing = parent;
    }
    let mut resolved = existing
        .canonicalize()
        .unwrap_or_else(|e| panic!("{}: {e}", existing.display()));
    resolved.extend(rest.iter().rev());
    resolved
}

fn listed(paths: &[PathBuf]) -> Vec<PathBuf> {
    paths.iter().map(|path| canonical(path)).collect()
}

/// The configuration git reads is every file of it, the ones that do not exist yet among
/// them: `$GIT_DIR/config` and `config.worktree`, an `include.path` whose target is missing,
/// an `includeIf` target whose condition does not hold, a nested include reached through
/// one that does, and `HEAD` where a condition names the branch. Caught by: listing only
/// the files gix loaded, which misses a target created later and a condition that starts
/// to hold.
#[test]
fn every_configuration_file_git_reads_is_named_even_one_that_does_not_exist() {
    let repo = Repo::new("inputs-config");
    let dot = repo.path().join(".git");
    repo.git(&["config", "--add", "include.path", "missing.config"]);
    repo.git(&[
        "config",
        "includeIf.onbranch:elsewhere.path",
        "branch.config",
    ]);
    repo.git(&["config", "--add", "include.path", "present.config"]);
    repo.write(
        ".git/present.config",
        b"[include]\n\tpath = nested.config\n",
    );

    let inputs = opened(&repo).diff_inputs();
    let configuration = listed(inputs.configuration());
    for expected in [
        "config",
        "config.worktree",
        "missing.config",
        "branch.config",
        "present.config",
        "nested.config",
        "HEAD",
    ] {
        let path = canonical(&dot.join(expected));
        assert!(
            configuration.contains(&path),
            "{} is not among {configuration:?}",
            path.display()
        );
    }

    let plain = Repo::new("inputs-config-plain");
    let configuration = listed(opened(&plain).diff_inputs().configuration());
    assert!(
        !configuration.contains(&canonical(&plain.path().join(".git/HEAD"))),
        "HEAD was named with no condition on the branch"
    );
}

/// The files every path's attributes and submodules come from: `info/attributes`, the
/// configured `core.attributesFile`, and the working tree's `.gitmodules`; and for one
/// path, the `.gitattributes` of every directory above it. Caught by: a list without the
/// configured attributes file (an edit to it would not be seen), or one that names the
/// working tree's attributes in a bare repository.
#[test]
fn the_attribute_files_git_reads_are_named_where_git_reads_them() {
    let repo = Repo::new("inputs-attributes");
    let configured = repo.path().join("elsewhere-attributes");
    repo.config("core.attributesFile", &configured.display().to_string());
    let inputs = opened(&repo).diff_inputs();
    let global = listed(inputs.global());
    for expected in [
        repo.path().join(".git/info/attributes"),
        configured,
        repo.path().join(".gitmodules"),
    ] {
        let path = canonical(&expected);
        assert!(
            global.contains(&path),
            "{} is not among {global:?}",
            path.display()
        );
    }
    let directories: Vec<PathBuf> =
        cairn_git::DiffInputs::directories(&cairn_model::RepoPath::from("a/b/c.txt"))
            .filter_map(|directory| inputs.attributes_in(directory))
            .map(|path| canonical(&path))
            .collect();
    assert_eq!(
        directories,
        [
            canonical(&repo.path().join(".gitattributes")),
            canonical(&repo.path().join("a/.gitattributes")),
            canonical(&repo.path().join("a/b/.gitattributes")),
        ]
    );

    let bare = Repo::new("inputs-bare");
    bare.git(&["config", "core.bare", "true"]);
    let bare = ok(
        Repository::discover(bare.path().join(".git")),
        "the bare repository opens",
    );
    assert_eq!(bare.diff_inputs().attributes_in(b""), None);
}

/// The index's part is what it holds, not its file: a stat-only `git update-index
/// --refresh` rewrites the file (its trailing checksum with it) and leaves it equal, and a
/// staged `.gitattributes` or `.gitmodules` edit does not; with no `.gitmodules` staged,
/// `HEAD`'s is the one read. Caught by: keying on the index file's checksum or stamp,
/// which a refresh moves, or leaving out an entry a diff reads.
#[test]
fn the_index_is_compared_by_what_it_holds_not_by_its_file() {
    let repo = Repo::new("inputs-staged");
    repo.write("a.txt", b"one\n");
    repo.write("sub/.gitattributes", b"*.bin -diff\n");
    repo.write(
        ".gitmodules",
        b"[submodule \"s\"]\n\tpath = s\n\turl = ./s\n",
    );
    repo.commit("seed");
    let index = repo.path().join(".git/index");
    let checksum = || {
        let bytes = std::fs::read(&index).unwrap_or_else(|e| panic!("reading the index: {e}"));
        bytes[bytes.len() - 20..].to_vec()
    };
    let before = ok(opened(&repo).staged_inputs(), "the index reads");
    let written = checksum();

    // A stat-only change: the file's time moves, its content does not.
    let file = std::fs::File::options()
        .write(true)
        .open(repo.path().join("a.txt"))
        .unwrap_or_else(|e| panic!("opening a.txt: {e}"));
    file.set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(5))
        .unwrap_or_else(|e| panic!("touching a.txt: {e}"));
    repo.git(&["update-index", "--refresh"]);
    assert_ne!(
        checksum(),
        written,
        "the refresh rewrote nothing to compare"
    );
    assert_eq!(
        ok(opened(&repo).staged_inputs(), "the index reads"),
        before,
        "a stat-only refresh changed what the index holds"
    );

    repo.write("sub/.gitattributes", b"*.bin -diff\n*.txt -diff\n");
    repo.git(&["add", "sub/.gitattributes"]);
    let attributes_staged = ok(opened(&repo).staged_inputs(), "the index reads");
    assert_ne!(
        attributes_staged, before,
        "a staged .gitattributes edit was not seen"
    );

    repo.write(
        ".gitmodules",
        b"[submodule \"s\"]\n\tpath = s\n\turl = ./t\n",
    );
    repo.git(&["add", ".gitmodules"]);
    let modules_staged = ok(opened(&repo).staged_inputs(), "the index reads");
    assert_ne!(
        modules_staged, attributes_staged,
        "a staged .gitmodules edit was not seen"
    );

    repo.git(&["rm", "--cached", "--quiet", ".gitmodules"]);
    let from_head = ok(opened(&repo).staged_inputs(), "the index reads");
    assert_ne!(
        from_head, modules_staged,
        "with none staged, HEAD's .gitmodules is the one git reads"
    );
    repo.git(&["commit", "--quiet", "-m", "drop .gitmodules"]);
    assert_ne!(
        ok(opened(&repo).staged_inputs(), "the index reads"),
        from_head,
        "HEAD's .gitmodules going was not seen"
    );
}

/// Opening again reads the configuration afresh, by the route the application opened it,
/// and shares the repository's log; a path that now names another repository is refused
/// rather than answered as this one. Caught by: a reopen that hands back the handle's
/// thread-local copy (the configuration gix read at the first open), one with a registry
/// of its own (closing the repository would not end its `git`), or one that opens
/// whatever the path finds.
#[test]
fn opening_again_reads_the_configuration_afresh_and_refuses_another_repository() {
    let repo = Repo::new("inputs-reopen");
    repo.write("dir/a.txt", b"one\n");
    let first = repo.commit("seed");
    let environment = |name: &str| std::env::var_os(name);
    let shared = ok(
        cairn_git::SharedRepository::discover_for(
            repo.path().join("dir"),
            super::git(),
            environment,
        ),
        "the fixture opens",
    );
    let included = repo.path().join(".git/later.config");
    repo.git(&["config", "--add", "include.path", "later.config"]);
    let named = |handle: &Repository| {
        listed(handle.diff_inputs().configuration()).contains(&canonical(&included))
    };
    assert!(
        !named(&shared.to_worker()),
        "the first open saw a later edit"
    );
    let reopened = ok(shared.reopen_for(super::git(), environment), "the reopen");
    assert!(
        named(&reopened),
        "the reopen read the configuration of the first open"
    );

    ok(
        reopened.changes(
            super::git(),
            &cairn_git::ChangesRequest::commit(first),
            &cairn_git::CancelSignal::new(),
        ),
        "a query on the reopened handle",
    );
    assert!(
        shared
            .command_log()
            .iter()
            .any(|record| record.arguments.iter().any(|a| a == "diff-tree")),
        "the reopened handle's git is not in the repository's log"
    );

    let nested = Repo::borrowed(&repo.path().join("dir"));
    nested.git(&["init", "--quiet", "."]);
    match shared.reopen_for(super::git(), environment) {
        Err(cairn_git::Error::RepositoryReplaced { .. }) => {}
        other => panic!("a path naming another repository was reopened: {other:?}"),
    }
}
