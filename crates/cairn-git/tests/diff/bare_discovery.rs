//! `safe.bareRepository`: a bare repository found by searching is opened exactly where the
//! git in use would open it from the same directory, with the same configuration, and a
//! planted one is refused before anything in it runs (`crate::bare_discovery` in
//! `cairn-git`, S1).

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use cairn_git::{CancelSignal, ChangesRequest, Error, SharedRepository};
use cairn_model::Oid;

use super::repositories::{Repo, empty_home};
use super::{git, ok};

/// Git `2.<minor>.0`.
fn since(minor: u32) -> cairn_git::ops::GitVersion {
    cairn_git::ops::GitVersion {
        major: 2,
        minor,
        patch: 0,
    }
}

/// A working tree with a commit, a linked worktree beside it, and a bare repository
/// planted inside it — a clone of it whose configuration points its working tree at the
/// enclosing one and names `core.fsmonitor`, the program every read with a working tree
/// runs. Every repository's own configuration says `safe.bareRepository = explicit`, which
/// git never reads from there.
struct Planted {
    work: Repo,
    worktree: PathBuf,
    planted: PathBuf,
    marker: PathBuf,
    head: Oid,
}

impl Planted {
    fn new(name: &str) -> Self {
        let work = Repo::new(name);
        work.write("a.txt", b"one\n");
        work.commit("first");
        work.write("a.txt", b"two\n");
        let head = work.commit("second");
        let worktree = work.path().with_extension("worktree");
        let _ = std::fs::remove_dir_all(&worktree);
        let worktree_text = worktree.display().to_string();
        work.git(&["worktree", "add", "--quiet", &worktree_text]);
        let planted = work.path().join("evil.git");
        let planted_text = planted.display().to_string();
        let work_text = work.path().display().to_string();
        work.git(&["clone", "--quiet", "--bare", &work_text, &planted_text]);
        let marker = work.path().with_extension("fsmonitor-ran");
        let _ = std::fs::remove_file(&marker);
        let trap = work.path().with_extension("trap.sh");
        ok(
            std::fs::write(
                &trap,
                format!("#!/bin/sh\necho ran >> '{}'\nexit 1\n", marker.display()),
            ),
            "writing the trap",
        );
        let in_planted = |args: &[&str]| {
            let mut all = vec!["--git-dir", planted_text.as_str()];
            all.extend_from_slice(args);
            work.git(&all);
        };
        in_planted(&["config", "core.bare", "false"]);
        in_planted(&["config", "core.worktree", ".."]);
        in_planted(&[
            "config",
            "core.fsmonitor",
            &format!("sh {}", trap.display()),
        ]);
        in_planted(&["config", "safe.bareRepository", "explicit"]);
        work.config("safe.bareRepository", "explicit");
        Self {
            work,
            worktree,
            planted,
            marker,
            head,
        }
    }

    fn ran(&self) -> bool {
        self.marker.exists()
    }
}

impl Drop for Planted {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.worktree);
        let _ = std::fs::remove_file(&self.marker);
        let _ = std::fs::remove_file(self.work.path().with_extension("trap.sh"));
    }
}

/// The launch environment both Cairn and the oracle `git` read: no system file, the empty
/// home, the global file `/dev/null`, then `extra` over them.
fn launch(extra: &[(&str, String)]) -> impl Fn(&str) -> Option<OsString> + use<> {
    let mut entries: Vec<(String, OsString)> = vec![
        ("GIT_CONFIG_NOSYSTEM".to_owned(), "1".into()),
        ("GIT_CONFIG_GLOBAL".to_owned(), "/dev/null".into()),
        ("HOME".to_owned(), empty_home().into()),
        ("XDG_CONFIG_HOME".to_owned(), empty_home().into()),
    ];
    for (name, value) in extra {
        entries.retain(|(held, _)| held != name);
        entries.push(((*name).to_owned(), value.into()));
    }
    move |name| {
        entries
            .iter()
            .find(|(held, _)| held == name)
            .map(|(_, value)| value.clone())
    }
}

/// Whether the git in use, run from `directory` in that environment, finds a repository.
fn git_opens(directory: &Path, extra: &[(&str, String)]) -> bool {
    let env: Vec<(&str, &str)> = extra
        .iter()
        .map(|(name, value)| (*name, value.as_str()))
        .collect();
    let (status, _, _) = Repo::borrowed(directory).run(&["rev-parse", "--git-dir"], &env, None);
    status.success()
}

/// A configuration file in the working tree, removed with it.
fn written(holder: &Path, name: &str, text: &str) -> String {
    let path = holder.join(format!("{name}.gitconfig"));
    ok(std::fs::write(&path, text), "writing a configuration file");
    path.display().to_string()
}

/// Every shape a repository can be found in, under every way the setting can be given and
/// not given, opens in Cairn exactly where the git in use opens it from the same directory
/// with the same configuration — the planted repository and a directory inside it, a
/// `.git` directory and a directory inside it entered directly, a linked worktree's git
/// directory, the worktree itself and the working tree — and on a git that has the setting
/// the fixture is shown to tell the shapes apart. The repositories' own configuration says
/// `explicit` throughout and is ignored, as git ignores it. Caught by: the setting read from
/// the repository's configuration, from `includeIf "gitdir:"`, or not from the command
/// line; a `.git` directory or a worktree's refused on a git that allows it, or allowed on
/// one that refuses it; and a value git dies on accepted.
#[test]
fn a_bare_repository_found_by_searching_opens_exactly_where_git_opens_it() {
    let fixture = Planted::new("bare-discovery");
    let holder = fixture.work.path();
    let explicit = written(holder, "explicit", "[safe]\n\tbareRepository = explicit\n");
    let overridden = written(
        holder,
        "overridden",
        "[safe]\n\tbareRepository = explicit\n[safe]\n\tbareRepository = all\n",
    );
    let invalid = written(holder, "invalid", "[safe]\n\tbareRepository = Explicit\n");
    let included = written(
        holder,
        "included",
        &format!("[include]\n\tpath = {explicit}\n"),
    );
    let conditional = written(
        holder,
        "conditional",
        &format!(
            "[includeIf \"gitdir:/\"]\n\tpath = {explicit}\n[includeIf \"gitdir:**\"]\n\tpath = {explicit}\n"
        ),
    );
    let global = |path: &str| vec![("GIT_CONFIG_GLOBAL", path.to_owned())];
    let settings: Vec<(&str, Vec<(&str, String)>)> = vec![
        (
            "unset (only the repositories' own say explicit)",
            Vec::new(),
        ),
        ("explicit in the global file", global(&explicit)),
        ("explicit, then all", global(&overridden)),
        ("a value git dies on", global(&invalid)),
        ("explicit through an include", global(&included)),
        ("explicit through includeIf gitdir", global(&conditional)),
        (
            "explicit on the command line",
            vec![(
                "GIT_CONFIG_PARAMETERS",
                "'safe.bareRepository=explicit'".to_owned(),
            )],
        ),
        (
            "explicit through GIT_CONFIG_COUNT",
            vec![
                ("GIT_CONFIG_COUNT", "1".to_owned()),
                ("GIT_CONFIG_KEY_0", "safe.bareRepository".to_owned()),
                ("GIT_CONFIG_VALUE_0", "explicit".to_owned()),
            ],
        ),
    ];
    let shapes: Vec<(&str, PathBuf)> = vec![
        ("the planted repository", fixture.planted.clone()),
        (
            "inside the planted repository",
            fixture.planted.join("refs"),
        ),
        ("a .git directory entered", holder.join(".git")),
        ("inside a .git directory", holder.join(".git/refs")),
        (
            "a linked worktree's git directory",
            holder.join(".git/worktrees").join(
                fixture
                    .worktree
                    .file_name()
                    .unwrap_or_else(|| panic!("the worktree has a name")),
            ),
        ),
        ("a linked worktree", fixture.worktree.clone()),
        ("a working tree", holder.to_owned()),
    ];

    let mut differ = Vec::new();
    let mut seen = std::collections::BTreeMap::new();
    for (setting, extra) in &settings {
        for (shape, directory) in &shapes {
            let expected = git_opens(directory, extra);
            let opened = SharedRepository::discover_for(directory, git(), launch(extra));
            if opened.is_ok() != expected {
                differ.push(format!(
                    "{shape}, {setting}: git opens it: {expected}, Cairn: {opened:?}"
                ));
            }
            match (&opened, *setting) {
                (Err(Error::InvalidConfig { .. }), "a value git dies on") => {}
                (Err(Error::BareRepositoryFoundBySearching { .. }), _) => {}
                (Err(other), _) => differ.push(format!("{shape}, {setting}: refused as {other}")),
                (Ok(_), _) => {}
            }
            seen.insert((*setting, *shape), expected);
        }
    }
    assert!(differ.is_empty(), "{}", differ.join("\n"));

    // The oracle is shown to decide something on this git.
    let version = git().version();
    let under = |setting: &str, shape: &str| seen[&(setting, shape)];
    let explicit_setting = "explicit in the global file";
    assert_eq!(
        under(explicit_setting, "the planted repository"),
        version < since(38),
        "git {version} and the planted repository"
    );
    assert_eq!(
        under(explicit_setting, "a .git directory entered"),
        version < since(38) || version >= since(44),
        "git {version} and a .git directory"
    );
    assert_eq!(
        under(explicit_setting, "a linked worktree's git directory"),
        version < since(38) || version >= since(45),
        "git {version} and a worktree's git directory"
    );
    for setting in [
        "unset (only the repositories' own say explicit)",
        "explicit, then all",
    ] {
        assert!(under(setting, "the planted repository"), "{setting}");
    }
    assert!(under(explicit_setting, "a working tree"));
    assert!(under(explicit_setting, "a linked worktree"));
}

/// The attack the setting exists for, end to end: a bare repository planted inside a
/// working tree, whose configuration names `core.fsmonitor`. Without the setting the user's
/// git opens it and runs the program on every read, and so does Cairn — the parity the
/// user decided (a read runs the repository's `core.fsmonitor`, as their own git does).
/// Under `safe.bareRepository = explicit` git refuses it, and Cairn refuses it as it is
/// opened, so no read starts and the program never runs. Caught by: the check skipped, or
/// made after the repository is in use.
#[test]
fn a_planted_bare_repository_is_refused_at_open_and_runs_nothing() {
    let fixture = Planted::new("bare-planted");
    let holder = fixture.work.path();
    let explicit = vec![(
        "GIT_CONFIG_GLOBAL",
        written(holder, "explicit", "[safe]\n\tbareRepository = explicit\n"),
    )];
    let request = ChangesRequest::commit(fixture.head);

    // Without the setting: opened, read, and the planted program runs, as under git.
    let shared = ok(
        SharedRepository::discover_for(&fixture.planted, git(), launch(&[])),
        "without the setting the planted repository opens, as git opens it",
    );
    let found = ok(
        shared
            .to_worker()
            .changes(git(), &request, &CancelSignal::new()),
        "the changes query answers",
    );
    assert_eq!(found.files.len(), 1, "{:?}", found.files);
    assert!(
        fixture.ran(),
        "the planted core.fsmonitor did not run on a read, so its absence below decides \
         nothing"
    );
    ok(std::fs::remove_file(&fixture.marker), "clearing the marker");

    if git().version() < since(38) {
        eprintln!(
            "SKIPPED a_planted_bare_repository_is_refused_at_open_and_runs_nothing: git {} \
             has no safe.bareRepository, and opens the planted repository whatever is set",
            git().version()
        );
        return;
    }
    let refused = SharedRepository::discover_for(&fixture.planted, git(), launch(&explicit));
    match refused {
        Err(Error::BareRepositoryFoundBySearching { path }) => assert_eq!(
            ok(std::fs::canonicalize(&path), "the refused path exists"),
            ok(
                std::fs::canonicalize(&fixture.planted),
                "the planted path exists"
            ),
        ),
        other => panic!("the planted repository was not refused: {other:?}"),
    }
    assert!(!fixture.ran(), "a program the planted repository names ran");
    let env: Vec<(&str, &str)> = explicit
        .iter()
        .map(|(name, value)| (*name, value.as_str()))
        .collect();
    assert!(
        Repo::borrowed(&fixture.planted)
            .try_git(&["status"], &env, None)
            .is_err(),
        "git opens the planted repository under explicit, so this test proves nothing"
    );
    assert!(!fixture.ran(), "git itself ran the planted program");
}
