//! Ownership at open: a repository git's own discovery refuses for dubious ownership is
//! refused as it is opened, exactly where the git in use refuses it from the same
//! directory with the same configuration, and nothing git opens is refused
//! (`crate::ownership` in `cairn-git`; its pure parts are unit-tested there).

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use cairn_git::{Error, SharedRepository};

use super::repositories::{Repo, empty_home};
use super::{git, ok, since};

/// A working tree with a subdirectory and a repository nested inside it, a linked worktree
/// and a bare clone beside it, and a link from outside to the working tree.
struct Owned {
    work: Repo,
    /// The working tree's top, every link resolved, as git compares it.
    top: PathBuf,
    worktree: PathBuf,
    bare: PathBuf,
    link: PathBuf,
}

impl Owned {
    fn new(name: &str) -> Self {
        let work = Repo::new(name);
        work.write("sub/a.txt", b"one\n");
        work.commit("first");
        let inner = Repo::borrowed(&work.path().join("inner"));
        ok(
            std::fs::create_dir_all(inner.path()),
            "making the nested repository",
        );
        inner.git(&["init", "--quiet", "--initial-branch=main", "."]);
        let worktree = work.path().with_extension("worktree");
        let _ = std::fs::remove_dir_all(&worktree);
        work.git(&[
            "worktree",
            "add",
            "--quiet",
            &worktree.display().to_string(),
        ]);
        let bare = work.path().with_extension("bare.git");
        let _ = std::fs::remove_dir_all(&bare);
        work.git(&[
            "clone",
            "--quiet",
            "--bare",
            &work.path().display().to_string(),
            &bare.display().to_string(),
        ]);
        let link = work.path().with_extension("link");
        let _ = std::fs::remove_file(&link);
        ok(
            std::os::unix::fs::symlink(work.path(), &link),
            "linking to the working tree",
        );
        let top = physical(work.path());
        Self {
            work,
            top,
            worktree,
            bare,
            link,
        }
    }
}

impl Drop for Owned {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.worktree);
        let _ = std::fs::remove_dir_all(&self.bare);
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
/// physical — or `None` when it refuses.
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
    ok(std::fs::canonicalize(path), "resolving a path")
}

/// A configuration file beside the working tree, removed with the directory it is in.
fn written(holder: &Path, name: &str, text: &str) -> String {
    let path = holder.join(format!("{name}.gitconfig"));
    ok(std::fs::write(&path, text), "writing a configuration file");
    path.display().to_string()
}

/// A global configuration every git reads: `~/.gitconfig` in a home of its own, which git
/// before 2.32 reads, and the same file named by `GIT_CONFIG_GLOBAL`, which git from 2.32
/// reads instead.
fn global_file(holder: &Path, name: &str, text: &str) -> Vec<(&'static str, String)> {
    let home = holder.join(format!("{name}.home"));
    ok(std::fs::create_dir_all(&home), "making a home");
    let file = written(&home, ".", text);
    let gitconfig = home.join(".gitconfig");
    ok(
        std::fs::rename(&file, &gitconfig),
        "naming the file .gitconfig",
    );
    vec![
        ("HOME", home.display().to_string()),
        ("GIT_CONFIG_GLOBAL", gitconfig.display().to_string()),
    ]
}

/// Every shape a repository is found in, under every way `safe.directory` can be given and
/// spelled, opens in Cairn exactly where the git in use opens it from the same directory
/// with the same configuration — the same git directory, or a refusal where git refuses —
/// with git told to take every path as someone else's (`GIT_TEST_ASSUME_DIFFERENT_OWNER`,
/// which git reads in every build from 2.30.4, and Cairn reads as git does), so the
/// setting decides. The shapes: the working tree's top, a directory inside it, a
/// repository nested inside it, a linked worktree, a bare repository and a link to the
/// working tree. The settings: none, `*`, a reset by the empty value either side of an
/// entry, the top exactly, with a trailing slash, through a link, `~/`, `%(prefix)/`,
/// `:(optional)`, relative, `.`, `<top>/*` and `<parent>/*`, through an include, on the
/// command line both ways, a name git cannot expand, and the test variable itself unset,
/// false, numeric and bogus. Caught by: gix's own `safe.directory` rule (which takes
/// `<top>/*` to name `<top>` itself and reads neither the command line, `.`, nor git's
/// normalisation), the test variable ignored, a refused repository opened with reduced
/// trust rather than refused, and any band of git's version table moved.
#[test]
fn a_repository_opens_exactly_where_git_opens_it_whatever_safe_directory_says() {
    let fixture = Owned::new("ownership-parity");
    let holder = fixture.work.path();
    let top = fixture.top.display().to_string();
    let parent = fixture
        .top
        .parent()
        .unwrap_or_else(|| panic!("the working tree has a parent"))
        .display()
        .to_string();
    let link = fixture.link.display().to_string();
    let entries = |values: &[&str]| {
        values
            .iter()
            .map(|value| format!("[safe]\n\tdirectory = {value}\n"))
            .collect::<String>()
    };
    let global = |name: &str, values: &[&str]| {
        let mut extra = vec![("GIT_TEST_ASSUME_DIFFERENT_OWNER", "1".to_owned())];
        extra.extend(global_file(holder, name, &entries(values)));
        extra
    };
    let included_file = written(holder, "included-target", &entries(&[&top]));
    let home_relative = format!(
        "~/{}",
        fixture
            .top
            .strip_prefix(&parent)
            .unwrap_or_else(|_| panic!("the top is under its parent"))
            .display()
    );
    let assume = |value: &str| vec![("GIT_TEST_ASSUME_DIFFERENT_OWNER", value.to_owned())];
    let mut settings: Vec<(&str, Vec<(&str, String)>)> = vec![
        ("the test variable unset", Vec::new()),
        ("someone else's, nothing set", assume("1")),
        ("the test variable false", assume("false")),
        ("the test variable empty", assume("")),
        ("the test variable 0x10", assume("0x10")),
        ("the test variable 1k", assume("1k")),
        ("the test variable bogus", assume("maybe")),
        ("*", global("star", &["*"])),
        ("* then reset", global("star-reset", &["*", ""])),
        ("reset then the top", global("reset-top", &["", &top])),
        ("the top", global("top", &[&top])),
        (
            "the top with a slash",
            global("top-slash", &[&format!("{top}/")]),
        ),
        ("a link to the top", global("link", &[&link])),
        ("~/ to the top", global("home", &[&home_relative])),
        (
            "%(prefix)/ before an absolute top",
            global("prefix", &[&format!("%(prefix)/{top}")]),
        ),
        (
            ":(optional) the top",
            global("optional", &[&format!(":(optional){top}")]),
        ),
        (
            ":(optional) a missing path, then *",
            global(
                "optional-missing",
                &[&format!(":(optional){top}/missing"), "*"],
            ),
        ),
        ("relative", global("relative", &["ownership-parity"])),
        (".", global("dot", &["."])),
        ("<top>/*", global("top-star", &[&format!("{top}/*")])),
        (
            "<parent>/*",
            global("parent-star", &[&format!("{parent}/*")]),
        ),
        (
            "a name git cannot expand",
            global("no-user", &["~cairn-no-such-user/x", "*"]),
        ),
        ("through an include", {
            let mut extra = vec![("GIT_TEST_ASSUME_DIFFERENT_OWNER", "1".to_owned())];
            extra.extend(global_file(
                holder,
                "include",
                &format!("[include]\n\tpath = {included_file}\n"),
            ));
            extra
        }),
        (
            "GIT_CONFIG_PARAMETERS",
            vec![
                ("GIT_TEST_ASSUME_DIFFERENT_OWNER", "1".to_owned()),
                // The old spelling, which every git reads: git before 2.31 stops on the
                // `'key'='value'` one for every command, which `bare_discovery`'s residuals
                // already state.
                ("GIT_CONFIG_PARAMETERS", format!("'safe.directory={top}'")),
            ],
        ),
        (
            "GIT_CONFIG_COUNT",
            vec![
                ("GIT_TEST_ASSUME_DIFFERENT_OWNER", "1".to_owned()),
                ("GIT_CONFIG_COUNT", "1".to_owned()),
                ("GIT_CONFIG_KEY_0", "safe.directory".to_owned()),
                ("GIT_CONFIG_VALUE_0", top.clone()),
            ],
        ),
    ];
    // HOME is where `~/` is expanded: the working tree's parent, for that setting alone,
    // whose file only a git from 2.32 reads, by `GIT_CONFIG_GLOBAL`.
    for (setting, extra) in &mut settings {
        if *setting == "~/ to the top" {
            extra.retain(|(name, _)| *name != "HOME");
            extra.push(("HOME", parent.clone()));
        }
    }
    let shapes: Vec<(&str, PathBuf)> = vec![
        ("the top", holder.to_owned()),
        ("a directory inside", holder.join("sub")),
        ("a nested repository", holder.join("inner")),
        ("a linked worktree", fixture.worktree.clone()),
        ("a bare repository", fixture.bare.clone()),
        ("a link to the top", fixture.link.clone()),
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
            match &opened {
                Err(Error::DubiousOwnership { .. } | Error::InvalidConfig { .. }) | Ok(_) => {}
                Err(other) => differ.push(format!("{shape}, {setting}: refused as {other}")),
            }
            seen.insert((*setting, *shape), expected.is_some());
        }
    }
    assert!(differ.is_empty(), "{}", differ.join("\n"));

    // The oracle is shown to decide something on this git: the variable is read from
    // 2.30.4 (and from the first release of each later line that has the check), so on
    // the floors and since it refuses what nothing names and opens what `*` names.
    let version = git().version();
    let under = |setting: &str, shape: &str| seen[&(setting, shape)];
    assert!(under("the test variable unset", "the top"));
    let reads_the_variable = version >= since(36)
        || matches!(
            (version.minor, version.patch),
            (30, 4..) | (31, 3..) | (32, 2..) | (33, 3..) | (34, 3..) | (35, 3..)
        );
    if reads_the_variable {
        assert!(
            !under("someone else's, nothing set", "the top"),
            "git {version}"
        );
        assert!(under("*", "the top"), "git {version}");
        assert!(!under("* then reset", "the top"), "git {version}");
        assert!(under("the top", "a directory inside"), "git {version}");
        assert!(!under("the top", "a nested repository"), "git {version}");
    }
    if version >= since(47) {
        // `<top>/*` names what is under the top, never the top itself.
        assert!(!under("<top>/*", "the top"), "git {version}");
        assert!(under("<top>/*", "a nested repository"), "git {version}");
        assert!(under(".", "the top") && !under(".", "a directory inside"));
    }
}

/// The privileged half, a stated review step rather than a gate step, because it needs a
/// second owner: as root, `cargo test -p cairn-git --test diff_engine -- --ignored
/// a_linked_worktree_whose_git_dir_is_someone_elses_is_refused_as_git_refuses_it`. A
/// linked worktree whose `.git` file and working tree are the user's but whose git
/// directory (`.git/worktrees/<name>`) is given to `nobody` (uid 65534) is refused by the
/// user's own `git status` there with "dubious ownership", and Cairn refuses to open it
/// with [`Error::DubiousOwnership`]. Caught by: trust taken from the `.git` file alone. A
/// `safe.directory` in root's own global configuration would let both through; the test
/// says so rather than passing.
#[test]
#[ignore = "needs root, to give a directory to another user"]
fn a_linked_worktree_whose_git_dir_is_someone_elses_is_refused_as_git_refuses_it() {
    let main = Repo::new("ownership");
    main.write("a.txt", b"one\n");
    main.commit("seed");
    let linked: PathBuf = main.path().with_extension("linked");
    let _ = std::fs::remove_dir_all(&linked);
    let _removed = Removed(linked.clone());
    main.git(&["worktree", "add", "--quiet", &linked.display().to_string()]);
    let name = linked
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let worktrees = main.path().join(".git/worktrees").join(name);
    assert!(worktrees.is_dir(), "{} is missing", worktrees.display());
    std::os::unix::fs::chown(&worktrees, Some(65534), Some(65534))
        .expect("giving a directory to nobody needs root: run this test as root");

    let (_, _, refusal) = Repo::borrowed(&linked).run(&["status"], &[], None);
    assert!(
        refusal.contains("dubious ownership"),
        "the user's own git does not refuse it, so this test decides nothing: {refusal}"
    );

    match SharedRepository::discover_for(&linked, git(), launch(&[])) {
        Err(Error::DubiousOwnership { .. }) => {}
        other => panic!("opened, or refused for another reason, where git refuses: {other:?}"),
    }
}

/// Removes a directory however the test ends, as a fixture `Repo` removes its own.
struct Removed(PathBuf);

impl Drop for Removed {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
