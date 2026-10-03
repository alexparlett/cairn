//! Parity of what stands in place of rows with what `git diff` prints (phase 07 QA, T4;
//! PRD R6.8, criterion C11): every state that is not text, built in a real repository by
//! real `git`, read by the engine, prepared as the diff thread prepares it
//! (`ShownDiff::new`) and turned into its notice (`DiffNotice::of`) — the whole road from
//! the repository to the words drawn, which only this crate links. Each notice's lines are
//! compared with the lines `git show` prints for the same file: the mode, rename and copy
//! lines of its extended header, a submodule's `Subproject commit` lines, an LFS pointer's
//! text, a binary's and a too-large file's sizes. Diverging from git is a critical bug, not
//! a gap.
//!
//! An integration test rather than a module of `src/`, so it may run `git` itself: the
//! guards hold `src/` to the rule that only `cairn-git`'s `process/` starts a process.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use cairn_git::ops::{Askpass, GitBinary};
use cairn_git::{CancelSignal, ChangesRequest, ContentOptions, Repository};
use cairn_model::{ChangedFile, Context, DiffLimits, Oid, ShownDiff, SizeLimit};
use cairn_ui::{
    COPIED_WITHOUT_CHANGES, DiffNotice, MODE_CHANGED, RENAMED_MODE_CHANGED,
    RENAMED_WITHOUT_CHANGES, subproject_line,
};

/// A repository in the temporary directory, removed when the test is done with it.
struct Fixture {
    path: PathBuf,
    home: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("cairn-{name}-{}", std::process::id()));
        let home = path.with_extension("home");
        for dir in [&path, &home] {
            let _ = std::fs::remove_dir_all(dir);
            if let Err(error) = std::fs::create_dir_all(dir) {
                panic!("making {}: {error}", dir.display());
            }
        }
        let fixture = Self { path, home };
        fixture.git(&["init", "--quiet"]);
        // Copies found as the user's own `git show` finds them when they ask for them.
        fixture.git(&["config", "diff.renames", "copies"]);
        fixture.git(&["config", "core.fileMode", "true"]);
        fixture
    }

    /// `git` in the repository, isolated from the machine's configuration, as `cairn-git`'s
    /// fixtures run it. Its standard output, or a panic naming git's complaint.
    fn git(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .current_dir(&self.path)
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("HOME", &self.home)
            .env("XDG_CONFIG_HOME", &self.home)
            .env("GIT_AUTHOR_NAME", "Ada")
            .env("GIT_AUTHOR_EMAIL", "ada@example.com")
            .env("GIT_COMMITTER_NAME", "Ada")
            .env("GIT_COMMITTER_EMAIL", "ada@example.com")
            .stdin(Stdio::null())
            .output()
            .unwrap_or_else(|error| panic!("starting git {args:?}: {error}"));
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).into_owned()
    }

    fn write(&self, path: &str, bytes: &[u8]) {
        let at = self.path.join(path);
        if let Err(error) = std::fs::write(&at, bytes) {
            panic!("writing {}: {error}", at.display());
        }
    }

    fn executable(&self, path: &str) {
        use std::os::unix::fs::PermissionsExt as _;
        let at = self.path.join(path);
        if let Err(error) = std::fs::set_permissions(&at, std::fs::Permissions::from_mode(0o755)) {
            panic!("making {} executable: {error}", at.display());
        }
    }

    /// Commits the working tree, with `submodule` pointing at `target` as a submodule does
    /// but with no checkout behind it: the tree records the commit alone, which is all a
    /// diff of it reads. Staged after the rest, since `add --all` would stage its missing
    /// checkout as a deletion.
    fn commit(&self, message: &str, (submodule, target): (&str, &str)) -> String {
        self.git(&["add", "--all"]);
        self.git(&[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{target},{submodule}"),
        ]);
        self.git(&["commit", "--quiet", "--message", message]);
        self.git(&["rev-parse", "HEAD"]).trim().to_owned()
    }

    fn size(&self, object: &str) -> u64 {
        self.git(&["cat-file", "-s", object])
            .trim()
            .parse()
            .unwrap_or_else(|error| panic!("the size of {object}: {error}"))
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
        let _ = std::fs::remove_dir_all(&self.home);
    }
}

/// One file's section of `git show`'s patch, by its new path: the extended header lines
/// between `diff --git` and the content, and every line after them.
#[derive(Debug, Default)]
struct Section {
    header: Vec<String>,
    body: Vec<String>,
}

fn sections(patch: &str) -> BTreeMap<String, Section> {
    let mut found: BTreeMap<String, Section> = BTreeMap::new();
    let mut current: Option<(String, bool)> = None;
    for line in patch.lines() {
        if let Some(paths) = line.strip_prefix("diff --git ") {
            let new = paths
                .rsplit_once(" b/")
                .map(|(_, new)| new.to_owned())
                .unwrap_or_else(|| panic!("a header git did not print: {line}"));
            found.insert(new.clone(), Section::default());
            current = Some((new, true));
            continue;
        }
        let Some((path, in_header)) = current.as_mut() else {
            continue;
        };
        let Some(section) = found.get_mut(path.as_str()) else {
            continue;
        };
        if *in_header
            && (line.starts_with("--- ")
                || line.starts_with("Binary files ")
                || line.starts_with("@@ "))
        {
            *in_header = false;
        }
        if *in_header {
            section.header.push(line.to_owned());
        } else {
            section.body.push(line.to_owned());
        }
    }
    found
}

/// The lines of a hunk body on one side, their markers taken off: context and `-` lines for
/// the old side, context and `+` lines for the new.
fn side_of(body: &[String], marker: char) -> Vec<String> {
    body.iter()
        .filter(|line| !line.starts_with("--- ") && !line.starts_with("+++ "))
        .filter(|line| !line.starts_with("@@ "))
        .filter_map(|line| {
            let mut chars = line.chars();
            match chars.next() {
                Some(' ') => Some(chars.as_str().to_owned()),
                Some(first) if first == marker => Some(chars.as_str().to_owned()),
                Some(_) | None => None,
            }
        })
        .collect()
}

fn pointer(oid_byte: char, size: u64) -> String {
    format!(
        "version https://git-lfs.github.com/spec/v1\noid sha256:{}\nsize {size}\n",
        std::iter::repeat_n(oid_byte, 64).collect::<String>()
    )
}

/// T4: every state that stands in place of rows, read from a real repository through the
/// engine and prepared as the diff thread prepares it, says what `git show` says of the
/// same file. A mode-only change, a rename and a copy with no content change, and a rename
/// whose mode moved draw git's own extended header lines, in git's order, under the user's
/// titles; a submodule bump draws git's two `Subproject commit` lines and is never called
/// binary; a binary draws git's two blob sizes; an LFS pointer draws each side's pointer
/// text as git prints it; a file past the drawing limit draws the size git records and
/// offers Load Diff. Caught by: a header line Cairn spells or orders differently from git
/// (the rename's mode lines after its rename lines; `copy` written `rename`), a submodule
/// titled binary, a size that is not the blob's, or a notice from another state.
#[test]
fn every_notice_says_what_git_diff_says_of_the_same_file() {
    let repo = Fixture::new("notice-parity");
    let lines =
        |n: usize, word: &str| -> String { (0..n).map(|i| format!("{word} line {i}\n")).collect() };
    repo.write("mode.sh", lines(6, "mode").as_bytes());
    repo.write("rename-me.txt", lines(8, "renamed").as_bytes());
    repo.write("source.txt", lines(10, "source").as_bytes());
    repo.write("moved.sh", lines(7, "moved").as_bytes());
    repo.write("image.bin", b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR old");
    repo.write("asset.psd", pointer('a', 12_345).as_bytes());
    repo.write("big.txt", b"small for now\n");
    let base = repo.commit("base", ("vendor/lib", &"1".repeat(40)));

    // One commit holding every state, as a user's commit would.
    repo.executable("mode.sh");
    repo.git(&["mv", "rename-me.txt", "renamed.txt"]);
    // A copy git finds with `-C`: its source is modified in the same commit, and the copy
    // is the source as it was.
    repo.write("copy.txt", lines(10, "source").as_bytes());
    repo.write("source.txt", lines(11, "source").as_bytes());
    repo.git(&["mv", "moved.sh", "moved-and-run.sh"]);
    repo.executable("moved-and-run.sh");
    repo.write("image.bin", b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR new and longer");
    repo.write("asset.psd", pointer('b', 67_890).as_bytes());
    let big = lines(30_000, "a line long enough to pass a mebibyte soon");
    assert!(big.len() as u64 > DiffLimits::MAX_BYTES);
    repo.write("big.txt", big.as_bytes());
    let head = repo.commit("every state", ("vendor/lib", &"2".repeat(40)));

    let patch = repo.git(&["show", "--format=", "--no-color", "--no-ext-diff", &head]);
    let git = sections(&patch);

    let helper = GitBinary::discover(&Askpass::new("/nonexistent/cairn-askpass", None))
        .unwrap_or_else(|error| panic!("finding git: {error}"));
    let engine = Repository::discover(&repo.path)
        .unwrap_or_else(|error| panic!("opening the fixture: {error}"));
    let id = Oid::parse(&head).unwrap_or_else(|error| panic!("{head}: {error}"));
    let request = ChangesRequest::commit(id);
    let changes = engine
        .changes(&helper, &request, &CancelSignal::new())
        .unwrap_or_else(|error| panic!("the changes query: {error}"));
    let notice = |path: &str| -> (ChangedFile, DiffNotice) {
        let file = changes
            .files
            .iter()
            .find(|file| file.new_path.display() == path)
            .unwrap_or_else(|| panic!("{path} is not among {:?}", changes.files))
            .clone();
        let diff = engine
            .file_diff(
                &helper,
                &request,
                &file,
                &ContentOptions::default(),
                &CancelSignal::new(),
            )
            .unwrap_or_else(|error| panic!("{path}'s diff: {error}"));
        let shown = ShownDiff::new(diff, Context::lines(3));
        let notice = DiffNotice::of(&shown).unwrap_or_else(|| panic!("{path} drew rows"));
        (file, notice)
    };
    let section = |path: &str| -> &Section {
        git.get(path)
            .unwrap_or_else(|| panic!("git printed nothing for {path}: {patch}"))
    };

    // The changes with no content to draw: git's whole extended header, line for line.
    for (path, title) in [
        ("mode.sh", MODE_CHANGED),
        ("renamed.txt", RENAMED_WITHOUT_CHANGES),
        ("copy.txt", COPIED_WITHOUT_CHANGES),
        ("moved-and-run.sh", RENAMED_MODE_CHANGED),
    ] {
        let (_, drawn) = notice(path);
        let wanted = DiffNotice::NoContentChange {
            title,
            lines: section(path).header.clone(),
        };
        assert_eq!(drawn, wanted, "{path}: git printed {:?}", section(path));
    }
    // What those headers hold, so the comparison above is over the lines that matter.
    assert_eq!(
        section("mode.sh").header,
        ["old mode 100644", "new mode 100755"]
    );
    assert!(
        section("copy.txt")
            .header
            .contains(&"copy from source.txt".to_owned()),
        "git found no copy: {patch}"
    );
    assert_eq!(
        section("moved-and-run.sh")
            .header
            .first()
            .map(String::as_str),
        Some("old mode 100644")
    );

    // A submodule bump: git's two lines, and never "binary".
    let (_, drawn) = notice("vendor/lib");
    let DiffNotice::Submodule { old, new, dirty } = drawn else {
        panic!("a submodule drew {drawn:?}");
    };
    let mut ours = Vec::new();
    ours.extend(old.map(|id| format!("-{}", subproject_line(id, false))));
    ours.extend(new.map(|id| format!("+{}", subproject_line(id, dirty))));
    let theirs: Vec<String> = section("vendor/lib")
        .body
        .iter()
        .filter(|line| line.starts_with("-Subproject") || line.starts_with("+Subproject"))
        .cloned()
        .collect();
    assert_eq!(ours, theirs, "the submodule's lines");
    assert_eq!(theirs.len(), 2);
    assert!(
        section("vendor/lib")
            .body
            .iter()
            .all(|line| !line.starts_with("Binary files")),
        "git itself called the submodule binary"
    );

    // A binary: git says so, and the sizes are the blobs'.
    let (_, drawn) = notice("image.bin");
    assert!(
        section("image.bin")
            .body
            .iter()
            .any(|line| line.starts_with("Binary files ")),
        "{:?}",
        section("image.bin")
    );
    assert_eq!(
        drawn,
        DiffNotice::Binary {
            old: Some(repo.size(&format!("{base}:image.bin"))),
            new: Some(repo.size(&format!("{head}:image.bin"))),
        }
    );

    // An LFS pointer: each side's text is the side git prints.
    let (_, drawn) = notice("asset.psd");
    let DiffNotice::LfsPointer { old, new } = drawn else {
        panic!("a pointer drew {drawn:?}");
    };
    let body = &section("asset.psd").body;
    let text_lines = |text: Option<String>| -> Vec<String> {
        text.unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    };
    assert_eq!(text_lines(old), side_of(body, '-'), "the old pointer");
    assert_eq!(text_lines(new), side_of(body, '+'), "the new pointer");

    // Past the drawing limit: the size git records, measured before anything is read.
    let (_, drawn) = notice("big.txt");
    assert_eq!(
        drawn,
        DiffNotice::TooLarge {
            crossed: SizeLimit::Bytes {
                limit: DiffLimits::MAX_BYTES,
                measured: repo.size(&format!("{head}:big.txt")),
            },
            loadable: true,
        }
    );
    assert!(
        section("big.txt")
            .body
            .iter()
            .any(|line| line.starts_with("@@ ")),
        "git drew the large file as text"
    );
}
