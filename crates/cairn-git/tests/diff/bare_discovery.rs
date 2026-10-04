//! `safe.bareRepository`: a bare repository found by searching is opened exactly where the
//! git in use would open it from the same directory, with the same configuration, and a
//! planted one is refused before anything in it runs (`crate::bare_discovery` in
//! `cairn-git`, S1).

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use cairn_git::{CancelSignal, ChangesRequest, Error, SharedRepository};
use cairn_model::Oid;

use super::repositories::{Repo, empty_home};
use super::{git, ok, since};

/// A working tree with a commit, a linked worktree beside it, and a bare repository
/// planted inside it — a clone of it whose configuration points its working tree at the
/// enclosing one and names `core.fsmonitor`, the program every read with a working tree
/// runs. Every repository's own configuration says `safe.bareRepository = explicit`, which
/// git never reads from there.
///
/// And two links: a second bare clone planted as `docs/`, holding `guide -> ../guide`, a
/// link to a plain directory of the working tree — so that the link's logical parent is
/// the planted repository and its physical parent the working tree — and a link from
/// outside the working tree to that same directory.
struct Planted {
    work: Repo,
    worktree: PathBuf,
    planted: PathBuf,
    marker: PathBuf,
    head: Oid,
    /// `docs/guide`, whose logical parent is the planted `docs/`.
    linked: PathBuf,
    /// A link beside the working tree to its `guide/`.
    link: PathBuf,
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
        let docs = work.path().join("docs");
        work.git(&[
            "clone",
            "--quiet",
            "--bare",
            &work_text,
            &docs.display().to_string(),
        ]);
        let guide = work.path().join("guide");
        ok(std::fs::create_dir(&guide), "making the linked directory");
        let linked = docs.join("guide");
        ok(
            std::os::unix::fs::symlink("../guide", &linked),
            "linking docs/guide to ../guide",
        );
        let link = work.path().with_extension("link");
        let _ = std::fs::remove_file(&link);
        ok(
            std::os::unix::fs::symlink(&guide, &link),
            "linking to the working tree from outside it",
        );
        Self {
            work,
            worktree,
            planted,
            marker,
            head,
            linked,
            link,
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
        let _ = std::fs::remove_file(&self.link);
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

/// The git directory the git in use, run from `directory` in that environment, opens —
/// physical — or `None` when it finds none or refuses it.
fn git_opens(directory: &Path, extra: &[(&str, String)]) -> Option<PathBuf> {
    let env: Vec<(&str, &str)> = extra
        .iter()
        .map(|(name, value)| (*name, value.as_str()))
        .collect();
    let (status, stdout, _) =
        Repo::borrowed(directory).run(&["rev-parse", "--absolute-git-dir"], &env, None);
    status
        .success()
        .then(|| physical(Path::new(stdout.trim_end())))
}

/// `path` with every link resolved.
fn physical(path: &Path) -> PathBuf {
    ok(std::fs::canonicalize(path), "resolving a git directory")
}

/// `GIT_CONFIG_COUNT` spelled `count`, its one entry saying `explicit`.
fn counted(count: &str) -> Vec<(&'static str, String)> {
    vec![
        ("GIT_CONFIG_COUNT", count.to_owned()),
        ("GIT_CONFIG_KEY_0", "safe.bareRepository".to_owned()),
        ("GIT_CONFIG_VALUE_0", "explicit".to_owned()),
    ]
}

/// A configuration file in the working tree, removed with it.
fn written(holder: &Path, name: &str, text: &str) -> String {
    let path = holder.join(format!("{name}.gitconfig"));
    ok(std::fs::write(&path, text), "writing a configuration file");
    path.display().to_string()
}

/// Every shape a repository can be found in, under every way the setting can be given and
/// not given, opens in Cairn exactly where the git in use opens it from the same directory
/// with the same configuration — the same git directory, or a refusal where git refuses —
/// for the planted repository and a directory inside it, a `.git` directory and a
/// directory inside it entered directly, a linked worktree's git directory, the worktree
/// itself and the working tree, a link inside a planted bare repository to a directory of
/// the working tree, and a link from outside to the same directory; and on a git that has
/// the setting the fixture is shown to tell the shapes apart. The repositories' own
/// configuration says `explicit` throughout and is ignored, as git ignores it. Caught by:
/// the setting read from the repository's configuration, from `includeIf "gitdir:"`, or
/// not from the command line; `GIT_CONFIG_COUNT` read other than as git's `strtoul` reads
/// it; a `.git` directory or a worktree's refused on a git that allows it, or allowed on
/// one that refuses it; a value git dies on accepted; and the repository opened found by
/// a search other than the one checked — one that follows the link logically opens the
/// planted `docs/`, which git never reaches from there.
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
        ("explicit through GIT_CONFIG_COUNT", counted("1")),
        // git reads the count with `strtoul`: each of these is a count it accepts.
        ("a count after whitespace", counted(" 1")),
        ("a count with a plus sign", counted("+1")),
        ("an empty count, which is none", counted("")),
        ("a count of minus zero", counted("-0")),
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
        (
            "a link in a planted repository to the working tree",
            fixture.linked.clone(),
        ),
        (
            "a link from outside to the working tree",
            fixture.link.clone(),
        ),
    ];

    let mut differ = Vec::new();
    let mut seen = std::collections::BTreeMap::new();
    for (setting, extra) in &settings {
        for (shape, directory) in &shapes {
            let expected = git_opens(directory, extra);
            let opened = SharedRepository::discover_for(directory, git(), launch(extra));
            let cairn_opens = opened
                .as_ref()
                .ok()
                .map(|shared| physical(shared.git_dir()));
            if cairn_opens != expected {
                differ.push(format!(
                    "{shape}, {setting}: git opens {expected:?}, Cairn {opened:?}"
                ));
            }
            match (&opened, *setting) {
                (Err(Error::InvalidConfig { .. }), "a value git dies on") => {}
                (Err(Error::BareRepositoryFoundBySearching { .. }), _) => {}
                (Err(other), _) => differ.push(format!("{shape}, {setting}: refused as {other}")),
                (Ok(_), _) => {}
            }
            seen.insert((*setting, *shape), expected.is_some());
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
    // The link's logical parent is a repository git opens, and not the one it opens from
    // the link: a search that followed the link logically would stop somewhere else.
    let working = git_opens(holder, &[]);
    assert!(working.is_some());
    for directory in [&fixture.linked, &fixture.link] {
        assert_eq!(
            git_opens(directory, &[]),
            working,
            "{}",
            directory.display()
        );
    }
    let logical_parent = git_opens(&holder.join("docs"), &[]);
    assert!(logical_parent.is_some() && logical_parent != working);
    // The counts are read, not ignored: two are one entry, two are none.
    for setting in ["a count after whitespace", "a count with a plus sign"] {
        assert_eq!(
            under(setting, "the planted repository"),
            version < since(38),
            "git {version}, {setting}"
        );
    }
    for setting in ["an empty count, which is none", "a count of minus zero"] {
        assert!(under(setting, "the planted repository"), "{setting}");
    }
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

/// Where the child of [`opening_reads_the_system_file_without_running_a_process`] is told
/// what to open, and where its recording `git` writes.
const OPEN_TARGET: &str = "CAIRN_TEST_OPEN_TARGET";
const RECORDED: &str = "CAIRN_TEST_RECORDED";

/// Reading the configuration git protects runs no process: the system file is named, not
/// asked of a `git` — which would run outside `GitEnvironment`, from the process's own
/// `PATH` and environment. This test binary is run again with a `git` first on `PATH`
/// that records every invocation, no `GIT_CONFIG_NOSYSTEM` (so the system file is
/// looked for), and the global file saying `explicit`; the child opens the planted
/// repository through `SharedRepository::discover`, the process's own environment, which
/// refuses it having read every file, under git 2.45's rule whichever git is installed —
/// and the recorder is shown to record by one invocation the child makes itself. Caught
/// by: gix's `Source::GitInstallation`, whose path gix-path finds by running
/// `git config -lz --show-origin`.
#[test]
fn opening_reads_the_system_file_without_running_a_process() {
    if let Some(target) = std::env::var_os(OPEN_TARGET) {
        let status = std::process::Command::new("git")
            .arg("recorder-check")
            .status();
        assert!(status.is_ok(), "the recording git did not run: {status:?}");
        let refused = SharedRepository::discover(PathBuf::from(target));
        assert!(
            matches!(refused, Err(Error::BareRepositoryFoundBySearching { .. })),
            "the planted repository was not refused under the global file: {refused:?}"
        );
        return;
    }
    let fixture = Planted::new("bare-no-process");
    let holder = fixture.work.path();
    let explicit = written(holder, "explicit", "[safe]\n\tbareRepository = explicit\n");
    let shim = holder.with_extension("shim");
    let _ = std::fs::remove_dir_all(&shim);
    ok(
        std::fs::create_dir(&shim),
        "making the recorder's directory",
    );
    let recorded = holder.with_extension("recorded");
    let _ = std::fs::remove_file(&recorded);
    let recorder = shim.join("git");
    ok(
        std::fs::write(
            &recorder,
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$CAIRN_TEST_RECORDED\"\nexit 1\n",
        ),
        "writing the recording git",
    );
    ok(
        std::fs::set_permissions(
            &recorder,
            std::os::unix::fs::PermissionsExt::from_mode(0o755),
        ),
        "making the recording git executable",
    );
    let path = std::env::join_paths(
        std::iter::once(shim.clone()).chain(
            std::env::var_os("PATH")
                .iter()
                .flat_map(std::env::split_paths),
        ),
    );
    let output = std::process::Command::new(ok(std::env::current_exe(), "this test binary"))
        .args([
            "--exact",
            "diff::bare_discovery::opening_reads_the_system_file_without_running_a_process",
            "--test-threads=1",
        ])
        .env_remove("GIT_CONFIG_NOSYSTEM")
        .env_remove("GIT_CONFIG_SYSTEM")
        .env_remove("GIT_CONFIG_PARAMETERS")
        .env_remove("GIT_CONFIG_COUNT")
        .env("PATH", ok(path, "joining PATH"))
        .env("HOME", empty_home())
        .env("XDG_CONFIG_HOME", empty_home())
        .env("GIT_CONFIG_GLOBAL", &explicit)
        .env(OPEN_TARGET, &fixture.planted)
        .env(RECORDED, &recorded)
        .output();
    let output = ok(output, "running this test binary again");
    let invocations = std::fs::read_to_string(&recorded).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&shim);
    let _ = std::fs::remove_file(&recorded);
    assert!(
        output.status.success(),
        "the child failed:\n{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("1 passed"),
        "the child ran no test:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(
        invocations, "recorder-check\n",
        "opening the repository ran a process"
    );
}

/// `GIT_CEILING_DIRECTORIES` and `GIT_DISCOVERY_ACROSS_FILESYSTEM` from the launch
/// environment both bound the search, so with each spelled as the user's own git reads
/// it Cairn opens exactly what that git opens from the same directory, or nothing where it
/// finds nothing: a working tree's repository and a bare one found by searching, from the
/// working tree, below it and inside the bare one, under a ceiling at, above and below the
/// repository, at the starting directory itself (never its own ancestor), at `/`, several
/// at once (the longest ancestor decides, a missing one is dropped), an empty entry (which
/// leaves every entry after it as written: a link or a `..` then names nothing), relative
/// entries (dropped), and a variable git cannot read as a boolean (git dies, Cairn
/// refuses). Every variable is set in every case, empty where it says nothing, so the
/// oracle inherits nothing of this process's own. Caught by: a search that ignores either
/// variable — it opens the enclosing repository git calls "not a git repository" and names
/// it with `--git-dir`, which no ceiling stops — counts the ceiling itself as searched,
/// resolves entries after an empty one, or accepts what git dies on.
#[test]
fn a_ceiling_stops_cairns_search_exactly_where_it_stops_gits() {
    let work = Repo::new("ceiling");
    work.write("a/b/file.txt", b"one\n");
    work.commit("first");
    let top = physical(work.path());
    let top_text = top.display().to_string();
    let bare = top.join("inner.git");
    work.git(&[
        "clone",
        "--quiet",
        "--bare",
        ".",
        &bare.display().to_string(),
    ]);
    ok(
        std::os::unix::fs::symlink(".", top.join("alias")),
        "linking alias to the working tree",
    );
    let alias = format!("{top_text}/alias");
    let parent = some_parent(&top);
    let starts: Vec<(&str, PathBuf)> = vec![
        ("the working tree", top.clone()),
        ("a directory below it", top.join("a/b")),
        ("the directory between", top.join("a")),
        ("inside the bare repository", bare.join("refs")),
    ];
    let ceilings: Vec<(&str, String)> = vec![
        ("none", String::new()),
        ("the working tree", top_text.clone()),
        ("the working tree with a slash", format!("{top_text}/")),
        ("the directory between", format!("{top_text}/a")),
        ("the deepest start", format!("{top_text}/a/b")),
        ("the bare repository", bare.display().to_string()),
        ("above the working tree", parent.clone()),
        ("the root", "/".to_owned()),
        (
            "a missing entry, then the working tree",
            format!("/nonexistent-cairn-ceiling/x:{top_text}"),
        ),
        (
            "above, then the directory between",
            format!("{parent}:{top_text}/a"),
        ),
        ("a link to the working tree", alias.clone()),
        ("an empty entry, then the link", format!(":{alias}")),
        ("the link, then an empty entry", format!("{alias}:")),
        (
            "an empty entry, then the working tree",
            format!(":{top_text}"),
        ),
        (
            "a .. that resolves to the working tree",
            format!("{top_text}/a/.."),
        ),
        ("an empty entry, then that ..", format!(":{top_text}/a/..")),
        ("relative entries", "a:..:.".to_owned()),
        ("a missing last component", format!("{top_text}/missing")),
        (
            "a missing middle component",
            format!("{top_text}/missing/deeper"),
        ),
    ];
    let across: Vec<(&str, &str)> = vec![
        ("unset", ""),
        ("true", "true"),
        ("a number", "2"),
        ("a value git dies on", "sometimes"),
    ];
    let mut differ = Vec::new();
    let mut seen = std::collections::BTreeMap::new();
    for (ceiling, entries) in &ceilings {
        for (crossing, value) in &across {
            let extra = vec![
                ("GIT_CEILING_DIRECTORIES", entries.clone()),
                ("GIT_DISCOVERY_ACROSS_FILESYSTEM", (*value).to_owned()),
            ];
            for (start, directory) in &starts {
                let expected = git_opens(directory, &extra);
                let opened = SharedRepository::discover_for(directory, git(), launch(&extra));
                let cairn_opens = opened
                    .as_ref()
                    .ok()
                    .map(|shared| physical(shared.git_dir()));
                if cairn_opens != expected {
                    differ.push(format!(
                        "from {start}, ceiling {ceiling}, across {crossing}: git opens \
                         {expected:?}, Cairn {opened:?}"
                    ));
                }
                if *crossing == "a value git dies on"
                    && !matches!(opened, Err(Error::InvalidConfig { ref key, .. }) if key == "GIT_DISCOVERY_ACROSS_FILESYSTEM")
                {
                    differ.push(format!(
                        "from {start}, ceiling {ceiling}: refused other than as git does: \
                         {opened:?}"
                    ));
                }
                seen.insert((*ceiling, *crossing, *start), expected.is_some());
            }
        }
    }
    assert!(differ.is_empty(), "{}", differ.join("\n"));
    // The oracle decides something: the ceiling at the working tree hides it from below,
    // and not from the working tree itself or with no ceiling.
    let under = |ceiling: &str, start: &str| seen[&(ceiling, "unset", start)];
    assert!(under("none", "a directory below it"));
    assert!(!under("the working tree", "a directory below it"));
    assert!(under("the working tree", "the working tree"));
    assert!(!under("the bare repository", "inside the bare repository"));
    assert!(under(
        "an empty entry, then the link",
        "a directory below it"
    ));
    assert!(!under("a link to the working tree", "a directory below it"));
    assert!(!seen[&("none", "a value git dies on", "the working tree")]);
}

/// The directory above `path`, as text.
fn some_parent(path: &Path) -> String {
    path.parent()
        .unwrap_or_else(|| panic!("{} has a parent", path.display()))
        .display()
        .to_string()
}

/// Where the child of
/// [`the_search_crosses_a_filesystem_boundary_exactly_where_git_crosses_it`] makes its
/// repository, in a mount namespace of its own.
const ACROSS_CHILD: &str = "CAIRN_TEST_ACROSS_FILESYSTEMS";

/// `GIT_DISCOVERY_ACROSS_FILESYSTEM`: a directory on another filesystem inside a working
/// tree finds no repository by default, as git stops at the boundary ("Stopping at
/// filesystem boundary"), and finds the enclosing one when the variable is true — every
/// spelling git reads as a boolean, each compared with the git in use. A second filesystem
/// without root takes a user and mount namespace (`unshare --map-root-user --mount`) with a
/// `tmpfs` mounted in the working tree, so this test binary runs itself again in one; where
/// there is none the test says so and decides nothing. Caught by: the variable ignored,
/// read as set when it is empty, or read other than as git reads a boolean.
#[test]
fn the_search_crosses_a_filesystem_boundary_exactly_where_git_crosses_it() {
    if std::env::var_os(ACROSS_CHILD).is_some() {
        across_filesystems_in_a_namespace();
        return;
    }
    let available = std::process::Command::new("unshare")
        .args(["--map-root-user", "--mount", "true"])
        .output()
        .is_ok_and(|output| output.status.success());
    if !available {
        eprintln!(
            "SKIPPED the_search_crosses_a_filesystem_boundary_exactly_where_git_crosses_it: \
             no user and mount namespace here, so no second filesystem"
        );
        return;
    }
    let output = std::process::Command::new("unshare")
        .args(["--map-root-user", "--mount"])
        .arg(ok(std::env::current_exe(), "this test binary"))
        .args([
            "--exact",
            "diff::bare_discovery::the_search_crosses_a_filesystem_boundary_exactly_where_git_crosses_it",
            "--test-threads=1",
            "--nocapture",
        ])
        .env(ACROSS_CHILD, "1")
        .output();
    let output = ok(output, "running this test binary in a namespace");
    assert!(
        output.status.success(),
        "the child failed:\n{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("1 passed"),
        "the child ran no test:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
}

/// The child's half: a `tmpfs` mounted at `mounted/` in a working tree, searched from a
/// directory in it under each value of the variable.
fn across_filesystems_in_a_namespace() {
    let work = Repo::new("across");
    work.write("a.txt", b"one\n");
    work.commit("first");
    let mount_point = physical(work.path()).join("mounted");
    ok(std::fs::create_dir(&mount_point), "making the mount point");
    let mounted = std::process::Command::new("mount")
        .args(["-t", "tmpfs", "cairn-test"])
        .arg(&mount_point)
        .status();
    assert!(
        mounted
            .as_ref()
            .is_ok_and(std::process::ExitStatus::success),
        "could not mount a tmpfs in the namespace: {mounted:?}"
    );
    let _unmounted = Unmounted(mount_point.clone());
    let start = mount_point.join("inside");
    ok(
        std::fs::create_dir(&start),
        "making a directory on the tmpfs",
    );
    use std::os::unix::fs::MetadataExt as _;
    assert_ne!(
        ok(std::fs::metadata(&start), "the start").dev(),
        ok(std::fs::metadata(work.path()), "the working tree").dev(),
        "the tmpfs is not another filesystem"
    );
    let mut differ = Vec::new();
    let mut opened_by_git = Vec::new();
    for value in [
        "",
        "0",
        "false",
        "no",
        "off",
        "1",
        "true",
        "YES",
        "On",
        "2",
        "-1",
        "0x10",
        "1k",
        "sometimes",
    ] {
        let extra = vec![
            ("GIT_CEILING_DIRECTORIES", String::new()),
            ("GIT_DISCOVERY_ACROSS_FILESYSTEM", value.to_owned()),
        ];
        let expected = git_opens(&start, &extra);
        let opened = SharedRepository::discover_for(&start, git(), launch(&extra));
        let cairn_opens = opened
            .as_ref()
            .ok()
            .map(|shared| physical(shared.git_dir()));
        if cairn_opens != expected {
            differ.push(format!(
                "{value:?}: git opens {expected:?}, Cairn {opened:?}"
            ));
        }
        opened_by_git.push((value, expected.is_some()));
    }
    assert!(differ.is_empty(), "{}", differ.join("\n"));
    // The oracle decides something: git stops at the boundary unless told to cross it.
    assert!(opened_by_git.contains(&("", false)), "{opened_by_git:?}");
    assert!(opened_by_git.contains(&("true", true)), "{opened_by_git:?}");
}

/// Unmounts the tmpfs before the working tree is removed, so its removal can finish.
struct Unmounted(PathBuf);

impl Drop for Unmounted {
    fn drop(&mut self) {
        let _ = std::process::Command::new("umount").arg(&self.0).status();
    }
}
