//! Ownership at open: a repository git's own discovery refuses for dubious ownership is
//! refused as it is opened, exactly where the git in use refuses it from the same
//! directory with the same configuration, and nothing git opens is refused
//! (`crate::ownership` in `cairn-git`; its pure parts are unit-tested there).

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use cairn_git::{CancelSignal, ChangesRequest, ContentOptions, Error, SharedRepository};
use cairn_model::{Context, DiffContent};

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
/// `:(optional)`, relative, `.`, `<top>/*` and `<parent>/*`, through an include, in a system
/// file under `GIT_CONFIG_NOSYSTEM` (which git 2.38.x alone reads), on the command line both
/// ways, a name git cannot expand, and the test variable itself unset, false, numeric and
/// bogus. Caught by: gix's own `safe.directory` rule (which takes
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
        (
            // Not a directory: `stat` fails with ENOTDIR, which `is_missing_file` does not
            // take as missing, so git 2.52 on stops.
            ":(optional) under a file, then *",
            global(
                "optional-under-a-file",
                &[&format!(":(optional){top}/sub/a.txt/x"), "*"],
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
            // git 2.38.x reads the system file even under GIT_CONFIG_NOSYSTEM, which every
            // other git honours.
            "the system file under GIT_CONFIG_NOSYSTEM",
            vec![
                ("GIT_TEST_ASSUME_DIFFERENT_OWNER", "1".to_owned()),
                (
                    "GIT_CONFIG_SYSTEM",
                    written(holder, "system", &entries(&["*"])),
                ),
            ],
        ),
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
    let nosystem_ignored = version.major == 2 && version.minor == 38;
    assert_eq!(
        under("the system file under GIT_CONFIG_NOSYSTEM", "the top"),
        nosystem_ignored || !reads_the_variable,
        "git {version}"
    );
    if version >= since(47) {
        // `<top>/*` names what is under the top, never the top itself.
        assert!(!under("<top>/*", "the top"), "git {version}");
        assert!(under("<top>/*", "a nested repository"), "git {version}");
        assert!(under(".", "the top") && !under(".", "a directory inside"));
    }
}

/// A `.git` git's search stops on stops Cairn's at open where the git in use stops, and is
/// passed over where it is passed over — so the enclosing repository it sits in opens in
/// Cairn exactly where git opens it: a `.git` file naming a directory that is not a
/// repository (git stops on every version), and a `.git` that is a FIFO (git 2.54 on stops,
/// older gits pass over it). Caught by: a `.git` gix cannot follow taken as no `.git`, which
/// opened the enclosing repository where git refuses.
#[test]
fn a_dot_git_git_stops_on_stops_cairn_where_it_stops_git() {
    let enclosing = Repo::new("dot-git-stops");
    enclosing.write("a.txt", b"one\n");
    enclosing.commit("seed");
    let empty = enclosing.path().join("empty");
    ok(std::fs::create_dir_all(&empty), "making an empty directory");
    let gitfile = enclosing.path().join("gitfile");
    ok(std::fs::create_dir_all(&gitfile), "making a working tree");
    ok(
        std::fs::write(
            gitfile.join(".git"),
            format!("gitdir: {}\n", empty.display()),
        ),
        "writing a .git file",
    );
    let fifo = enclosing.path().join("fifo");
    ok(std::fs::create_dir_all(&fifo), "making a working tree");
    let made = std::process::Command::new("mkfifo")
        .arg(fifo.join(".git"))
        .status();
    assert!(
        made.as_ref().is_ok_and(std::process::ExitStatus::success),
        "mkfifo: {made:?}"
    );
    let mut stopped = Vec::new();
    for directory in [&gitfile, &fifo] {
        let expected = git_opens(directory, &[]);
        let opened = SharedRepository::discover_for(directory, git(), launch(&[]));
        match (&expected, &opened) {
            (Some(git_dir), Ok(shared)) => assert_eq!(physical(shared.git_dir()), *git_dir),
            (None, Err(Error::NotARepository { path })) => {
                assert_eq!(*path, physical(directory).join(".git"));
                stopped.push(directory);
            }
            _ => panic!(
                "{}: git opens {expected:?}, Cairn {opened:?}",
                directory.display()
            ),
        }
    }
    assert!(
        stopped.contains(&&gitfile),
        "git opened past a .git file naming no repository"
    );
    assert_eq!(
        stopped.contains(&&fifo),
        git().version() >= since(54),
        "git {}",
        git().version()
    );
}

/// A `.git` file is read as git's `read_gitfile_gently` reads it (the same rules at every tag
/// from v2.30.0 to v2.56.0, moved into the `read_gitfile_raw` it calls at v2.56.0): at most
/// 1 MiB, `gitdir: ` and a path, only trailing newlines and
/// carriage returns taken off, so a trailing space or tab is part of the path. git stops on
/// a file it cannot follow ("too large to be a .git file", "not a git repository"), and so
/// does Cairn; where it can, both open the repository it names. Caught by: gix's reading of
/// the file in the search, which trims every trailing blank (opening a repository git
/// stops on) and reads at most 64 KiB.
///
/// The residual, ACCEPTED by the user on 2026-10-04 and pinned so that a change in gix shows
/// here: a file git CAN follow that gix
/// cannot — padded past gix-discover 0.55's 64 KiB, or with a NUL after the path, which
/// git's C string ends at — is one git opens and Cairn refuses, because gix reads the file
/// again as it opens and no open option hands it the git directory instead without
/// changing which working tree it assigns. And a path ending in a blank that names a
/// repository of its own is that repository to git and the trimmed one to gix: Cairn
/// refuses it as `Error::RepositoryReplaced`, the git directory judged not the one opened.
#[test]
fn a_dot_git_file_is_read_as_git_reads_it() {
    const SPACED: &str = "a trailing space naming a repository that exists";
    let target = Repo::new("dot-git-target");
    target.write("a.txt", b"one\n");
    target.commit("seed");
    let named = physical(&target.path().join(".git")).display().to_string();
    let holder = Repo::new("dot-git-holder");
    let padded = |total: usize| {
        let mut text = format!("gitdir: {named}").into_bytes();
        text.resize(total, b'\n');
        text
    };
    let shapes: Vec<(&str, Vec<u8>)> = vec![
        ("plain", format!("gitdir: {named}\n").into_bytes()),
        (
            "a carriage return",
            format!("gitdir: {named}\r\n").into_bytes(),
        ),
        (
            "a trailing space",
            format!("gitdir: {named} \n").into_bytes(),
        ),
        ("a trailing tab", format!("gitdir: {named}\t").into_bytes()),
        ("padded under 64 KiB", padded(60 * 1024)),
        ("padded past 1 MiB", padded((1 << 20) + 1)),
    ];
    let residual: Vec<(&str, Vec<u8>)> = vec![
        ("padded to exactly 1 MiB", padded(1 << 20)),
        (
            "a NUL after the path",
            format!("gitdir: {named}\0junk\n").into_bytes(),
        ),
        (SPACED, format!("gitdir: {named} \n").into_bytes()),
    ];
    let mut differ = Vec::new();
    let mut seen = std::collections::BTreeMap::new();
    for (index, (shape, text)) in shapes.iter().chain(residual.iter()).enumerate() {
        if *shape == SPACED {
            // Made only now, after the trailing space above was asked about naming nothing.
            target.git(&["init", "--quiet", "--bare", &format!("{named} ")]);
        }
        let directory = holder.path().join(format!("tree-{index}"));
        ok(std::fs::create_dir_all(&directory), "making a working tree");
        ok(
            std::fs::write(directory.join(".git"), text),
            "writing a .git file",
        );
        let expected = git_opens(&directory, &[]);
        let opened = SharedRepository::discover_for(&directory, git(), launch(&[]));
        let cairn_opens = opened
            .as_ref()
            .ok()
            .map(|shared| physical(shared.git_dir()));
        seen.insert(*shape, expected.is_some());
        let residual = residual.iter().any(|(name, _)| name == shape);
        if residual {
            let replaced = matches!(opened, Err(Error::RepositoryReplaced { .. }));
            if expected.is_none() || cairn_opens.is_some() || (*shape == SPACED) != replaced {
                differ.push(format!(
                    "{shape}: the residual moved — git opens {expected:?}, Cairn {opened:?}"
                ));
            }
        } else if cairn_opens != expected {
            differ.push(format!("{shape}: git opens {expected:?}, Cairn {opened:?}"));
        }
    }
    assert!(differ.is_empty(), "{}", differ.join("\n"));
    // The oracle decides something: git follows the plain file and stops on the others.
    assert!(seen["plain"]);
    assert!(seen["a carriage return"]);
    assert!(!seen["a trailing space"]);
    assert!(!seen["padded past 1 MiB"]);
}

/// A repository Cairn admits is read as git reads it, whatever gix's own owner rule makes of
/// it. gix checks the WORKING TREE's owner again as it opens — the directory `core.worktree`
/// names, not the one holding `.git` that git checks — and, refused by its own
/// `safe.directory` reading (system and global files only, compared as written), lowers the
/// repository to reduced trust; `Options::with(Trust::Full)` does not stop it
/// (gix 0.87.1, `src/open/repository.rs`, `open_from_paths`). Here the working tree is `/`,
/// which root owns, so every open below is one gix trusts less than fully while git's check
/// — of the directory holding `.git` — passes, under each way the launch environment admits
/// a repository git is told is someone else's: nothing, `safe.directory=*` on the command
/// line, and a global file naming the top with a trailing slash, or as `.`, where the git in
/// use admits them (its own answer is the oracle). Each open must then answer a `git`-backed
/// read (the changes query), a gix read of a blob past gix's reduced-trust allocation limit
/// (16 MiB, loaded anyway), and the repository's own `diff.context`. gix reads
/// `safe.directory` from the system and global files of the process it runs in, which a
/// test cannot choose for an open in its own process — a runner image's `/etc/gitconfig`
/// carrying `safe.directory = *` (GitHub's Ubuntu images do) trusts every open fully and
/// this decides nothing — so the opens run in this test binary again, in a child whose
/// system file is off and whose global file and home are empty, and the child shows gix's
/// trust reduced before it opens anything. Caught by: a reduced repository left to git's
/// discovery, which from `/` finds no repository at all, or read by gix under the limit it
/// gives reduced trust.
#[test]
fn a_repository_cairn_admits_is_read_as_git_reads_it_whatever_gix_makes_of_its_owner() {
    use std::os::unix::fs::MetadataExt as _;
    if std::env::var_os(REDUCED_CHILD).is_some() {
        read_as_git_reads_it_in_an_isolated_process();
        return;
    }
    let scratch = Repo::new("ownership-reduced");
    let probe = scratch.path().join("owner-probe");
    ok(std::fs::write(&probe, b""), "writing a probe");
    let me = ok(std::fs::symlink_metadata(&probe), "reading the probe").uid();
    if ok(std::fs::symlink_metadata("/"), "reading /").uid() == me {
        eprintln!(
            "SKIPPED a_repository_cairn_admits_is_read_as_git_reads_it_whatever_gix_makes_of_its_owner: \
             / is this user's own, so gix trusts it fully and this decides nothing"
        );
        return;
    }
    let name = module_path!()
        .split_once("::")
        .map(|(_, path)| {
            format!(
                "{path}::a_repository_cairn_admits_is_read_as_git_reads_it_whatever_gix_makes_of_its_owner"
            )
        })
        .unwrap_or_else(|| panic!("{} names no module", module_path!()));
    let output = ok(
        std::process::Command::new(ok(std::env::current_exe(), "this test binary"))
            .args(["--exact", &name, "--test-threads=1", "--nocapture"])
            .env(REDUCED_CHILD, "1")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("HOME", empty_home())
            .env("XDG_CONFIG_HOME", empty_home())
            .output(),
        "running this test binary with gix's configuration isolated",
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "the child failed:\n{stdout}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        stdout.contains("1 passed"),
        "the child ran no test:\n{stdout}"
    );
}

/// Marks the child
/// [`a_repository_cairn_admits_is_read_as_git_reads_it_whatever_gix_makes_of_its_owner`] runs,
/// with gix's configuration sources isolated.
const REDUCED_CHILD: &str = "CAIRN_TEST_REDUCED_TRUST_CHILD";

/// The child's half: every open of a repository whose working tree is `/`, under each way
/// the launch environment admits it, read as git reads it.
fn read_as_git_reads_it_in_an_isolated_process() {
    let scratch = Repo::new("ownership-reduced");
    let repo = Repo::new("ownership-work-tree-elsewhere");
    let mut big = Vec::with_capacity(17 * 1024 * 1024 + 1024);
    while big.len() <= 17 * 1024 * 1024 {
        big.extend_from_slice(&[b'z'; 1023]);
        big.push(b'\n');
    }
    repo.write("big.txt", &big);
    repo.write("small.txt", b"one\n");
    let head = repo.commit("seed");
    repo.config("diff.context", "7");
    repo.config("core.worktree", "/");
    // The reproduction holds: gix, by its own rule and in this process's configuration,
    // trusts the repository less than fully.
    assert_eq!(
        ok(gix::open(repo.path()), "opening with gix").git_dir_trust(),
        gix::sec::Trust::Reduced,
        "gix trusted a working tree root owns, so this decides nothing"
    );
    let top = physical(repo.path()).display().to_string();
    let assumed = |extra: &[(&'static str, String)]| {
        let mut all = vec![("GIT_TEST_ASSUME_DIFFERENT_OWNER", "1".to_owned())];
        all.extend_from_slice(extra);
        all
    };
    let settings: Vec<(&str, Vec<(&str, String)>)> = vec![
        ("the user's own", Vec::new()),
        (
            "safe.directory=* on the command line",
            assumed(&[("GIT_CONFIG_PARAMETERS", "'safe.directory=*'".to_owned())]),
        ),
        (
            "the top with a trailing slash",
            assumed(&global_file(
                scratch.path(),
                "slash",
                &format!("[safe]\n\tdirectory = {top}/\n"),
            )),
        ),
        (
            ".",
            assumed(&global_file(
                scratch.path(),
                "dot",
                "[safe]\n\tdirectory = .\n",
            )),
        ),
    ];
    let mut admitted = 0;
    for (setting, extra) in &settings {
        if git_opens(repo.path(), extra).is_none() {
            continue;
        }
        admitted += 1;
        let shared = match SharedRepository::discover_for(repo.path(), git(), launch(extra)) {
            Ok(shared) => shared,
            Err(error) => panic!("{setting}: git opens it and Cairn refuses: {error}"),
        };
        assert_eq!(shared.workdir(), Some(Path::new("/")), "{setting}");
        let engine = shared.to_worker();
        let request = ChangesRequest::commit(head);
        let changes = match engine.changes(git(), &request, &CancelSignal::new()) {
            Ok(changes) => changes,
            Err(error) => panic!("{setting}: the changes query fails where git answers: {error}"),
        };
        let paths: Vec<String> = changes
            .files
            .iter()
            .map(|file| file.new_path.display().to_string())
            .collect();
        assert_eq!(paths, ["big.txt", "small.txt"], "{setting}");
        let anyway = ContentOptions {
            load_anyway: true,
            ..ContentOptions::default()
        };
        let big_file = &changes.files[0];
        match engine.file_diff(git(), &request, big_file, &anyway, &CancelSignal::new()) {
            Ok(diff) => assert!(
                matches!(diff.content, DiffContent::Text { .. }),
                "{setting}: {:?}",
                diff.content
            ),
            Err(error) => panic!("{setting}: a 17 MiB blob git reads is refused: {error}"),
        }
        assert_eq!(
            ok(engine.configured_context(), "diff.context reads"),
            Context::Lines(7),
            "{setting}: the repository's own configuration was not read"
        );
    }
    // The user's own repository is admitted on every git, so this decided something; the
    // others are admitted only where the git in use reads them.
    assert!(admitted >= 1);
    let version = git().version();
    if version >= since(47) {
        assert_eq!(admitted, settings.len(), "git {version}");
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
