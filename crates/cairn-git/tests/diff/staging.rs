//! staging-and-commit's C3 and C4: a selection of lines staged, unstaged and discarded with
//! real `git apply`, as R3.1-R3.3 will run it — `git apply --cached --whitespace=nowarn -` to
//! stage and to unstage, `git apply --whitespace=nowarn -` to discard, never `-R` — over the
//! engine's own working-tree diffs and the model's [`action_patch`].
//!
//! Each result is held to three oracles, none of which is the code under test:
//!
//! 1. the reference applier ([`apply_patch`]), applying the same patch to the side it was
//!    built for — the index for stage and unstage, the working tree in git's form for
//!    discard;
//! 2. an independent derivation from the FORWARD diff and the selection, written here and
//!    never inverting anything: for stage, the forward rule read straight off the changed
//!    ranges; for unstage and discard, git's mirrored rule (`patch-mechanics-spike.md` E1b)
//!    — a whole-file patch of the original diff whose unselected additions are context and
//!    whose unselected removals are dropped — applied in reverse, by real `git apply -R` and
//!    by the reference applier, which must agree;
//! 3. what `git diff` (or `git diff --cached`) prints afterwards, whose changed lines must be
//!    exactly the lines the selection left out.
//!
//! Stage and unstage must leave the working tree's bytes alone and discard the index entry.
//! Every case runs on the host's git and, through `scripts/git-floor.sh`, on 2.30.9 and
//! 2.32.7. The state each apply starts from is restored byte for byte between applies.

use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt as _;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use cairn_git::{CancelSignal, ContentOptions, Repository, WorkingTreeDiff};
use cairn_model::{
    ChangeStatus, DiffContent, DiffLine, FileDiff, LineNumber, PatchAction, RepoPath, Selection,
    TextDiff, action_patch, apply_patch, apply_patch_in_reverse,
};

use super::ok;
use super::repositories::{Repo, empty_home};

/// `git` with byte arguments, isolated from the machine as `Repo::run` isolates it. `Ok` is
/// stdout; `Err` is the status and stderr.
fn git_bytes(dir: &Path, args: &[&[u8]], stdin: Option<&[u8]>) -> Result<Vec<u8>, String> {
    let mut command = Command::new("git");
    command
        .current_dir(dir)
        .args(args.iter().map(|arg| OsStr::from_bytes(arg)))
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("HOME", empty_home())
        .env("XDG_CONFIG_HOME", empty_home())
        .env("LC_ALL", "C")
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .unwrap_or_else(|e| panic!("could not start git: {e}"));
    if let Some(bytes) = stdin {
        use std::io::Write;
        let Some(mut pipe) = child.stdin.take() else {
            panic!("git was given no standard input");
        };
        pipe.write_all(bytes)
            .unwrap_or_else(|e| panic!("writing to git: {e}"));
    }
    let output = child
        .wait_with_output()
        .unwrap_or_else(|e| panic!("waiting for git: {e}"));
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(format!(
            "{} {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

/// `git diff`'s answer whatever its status: `--no-index` exits 1 with one.
fn git_diff_bytes(dir: &Path, args: &[&[u8]]) -> Vec<u8> {
    let mut command = Command::new("git");
    let output = command
        .current_dir(dir)
        .args(args.iter().map(|arg| OsStr::from_bytes(arg)))
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("HOME", empty_home())
        .env("XDG_CONFIG_HOME", empty_home())
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .output()
        .unwrap_or_else(|e| panic!("could not run git diff: {e}"));
    assert!(
        output.status.code().is_some_and(|code| code <= 1),
        "git {:?}: {} {}",
        args.iter()
            .map(|arg| String::from_utf8_lossy(arg))
            .collect::<Vec<_>>(),
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

fn on_disk(repo: &Repo, path: &[u8]) -> PathBuf {
    repo.path().join(OsStr::from_bytes(path))
}

fn write(repo: &Repo, path: &[u8], content: &[u8]) {
    std::fs::write(on_disk(repo, path), content)
        .unwrap_or_else(|e| panic!("writing {}: {e}", String::from_utf8_lossy(path)));
}

fn chmod(repo: &Repo, path: &[u8], mode: u32) {
    std::fs::set_permissions(on_disk(repo, path), std::fs::Permissions::from_mode(mode))
        .unwrap_or_else(|e| panic!("chmod {}: {e}", String::from_utf8_lossy(path)));
}

/// `lines` joined, each ended by `ending`.
fn text_of(lines: &[&str], ending: &str) -> Vec<u8> {
    let mut out = Vec::new();
    for line in lines {
        out.extend_from_slice(line.as_bytes());
        out.extend_from_slice(ending.as_bytes());
    }
    out
}

/// Twenty numbered lines: the content `HEAD` has.
fn head_lines() -> Vec<String> {
    (0..20).map(|n| format!("line {n:02}")).collect()
}

/// `HEAD`'s lines with the staged edits: a line replaced by two, a line removed, a line
/// added — three changes, the first two in one hunk.
fn index_lines() -> Vec<String> {
    let mut lines = head_lines();
    lines.splice(3..4, ["staged 03a".to_owned(), "staged 03b".to_owned()]);
    // `line 10` is now at 11.
    lines.remove(11);
    // `line 15` is now at 15.
    lines.insert(16, "staged after 15".to_owned());
    lines
}

/// The index's lines with the working tree's edits on top: a line replaced, a line
/// replaced by two, a line removed, a line added at the end.
fn work_lines() -> Vec<String> {
    let mut lines = index_lines();
    lines[1] = "work 01".to_owned();
    let at = lines
        .iter()
        .position(|line| line == "line 07")
        .unwrap_or_else(|| panic!("the fixture lost line 07"));
    lines.splice(at..=at, ["work 07a".to_owned(), "work 07b".to_owned()]);
    let at = lines
        .iter()
        .position(|line| line == "line 12")
        .unwrap_or_else(|| panic!("the fixture lost line 12"));
    lines.remove(at);
    lines.push("work end".to_owned());
    lines
}

fn joined(lines: &[String], ending: &str) -> Vec<u8> {
    text_of(
        &lines.iter().map(String::as_str).collect::<Vec<_>>(),
        ending,
    )
}

/// One case of C3: a repository in a state with something to stage, unstage or discard at
/// `path`, and the actions that apply to it.
struct Case {
    name: String,
    repo: Repo,
    /// The path the selection is made on: a rename's destination.
    path: Vec<u8>,
    /// A staged rename's or copy's source, which `git diff --cached` names beside the
    /// destination.
    renamed_from: Option<Vec<u8>>,
    /// Not in the index at all: its diff is the untracked one, and what `git diff` shows of
    /// it afterwards is `--no-index`'s.
    untracked: bool,
    actions: Vec<PatchAction>,
}

impl Case {
    fn new(name: &str, path: &[u8]) -> Self {
        let repo = Repo::new(&format!("c3-{}", name.replace(' ', "-")));
        Self {
            name: name.to_owned(),
            repo,
            path: path.to_vec(),
            renamed_from: None,
            untracked: false,
            actions: vec![
                PatchAction::Stage,
                PatchAction::Unstage,
                PatchAction::Discard,
            ],
        }
    }

    fn dir(&self) -> &Path {
        self.repo.path()
    }

    fn engine(&self) -> Repository {
        ok(Repository::discover(self.dir()), "the fixture opens")
    }

    /// The engine's diff of the path that `action` is made on: the staged one for unstage,
    /// the unstaged (or untracked) one otherwise.
    fn drawn(&self, action: PatchAction) -> FileDiff {
        let which = match action {
            PatchAction::Unstage => WorkingTreeDiff::Staged,
            PatchAction::Stage | PatchAction::Discard if self.untracked => {
                WorkingTreeDiff::Untracked
            }
            PatchAction::Stage | PatchAction::Discard => WorkingTreeDiff::Unstaged,
        };
        let answer = ok(
            self.engine().working_tree_diff(
                super::git(),
                &RepoPath::new(self.path.clone()),
                which,
                &ContentOptions {
                    load_anyway: true,
                    ..ContentOptions::default()
                },
                &CancelSignal::new(),
            ),
            &format!("{}: the {which:?} diff", self.name),
        );
        answer.unwrap_or_else(|| panic!("{}: no {which:?} diff to select from", self.name))
    }

    /// What the index holds at `path`: its mode and blob id, or `None`.
    fn staged(&self, path: &[u8]) -> Option<(String, String)> {
        let listed = git_bytes(
            self.dir(),
            &[
                b"--literal-pathspecs",
                b"ls-files",
                b"-s",
                b"-z",
                b"--",
                path,
            ],
            None,
        )
        .unwrap_or_else(|e| panic!("{}: ls-files: {e}", self.name));
        let record = listed.split(|byte| *byte == 0).next()?;
        let text = String::from_utf8_lossy(record);
        let mut fields = text.split_whitespace();
        Some((fields.next()?.to_owned(), fields.next()?.to_owned()))
    }

    fn blob(&self, id: &str) -> Vec<u8> {
        git_bytes(self.dir(), &[b"cat-file", b"blob", id.as_bytes()], None)
            .unwrap_or_else(|e| panic!("{}: cat-file {id}: {e}", self.name))
    }

    /// The working-tree file's id in git's form — through its clean filter and line-ending
    /// conversion — or `None` when there is no file.
    fn work_tree_id(&self) -> Option<String> {
        let bytes = std::fs::read(on_disk(&self.repo, &self.path)).ok()?;
        let mut path = b"--path=".to_vec();
        path.extend_from_slice(&self.path);
        let id = git_bytes(
            self.dir(),
            &[b"hash-object", b"--stdin", &path],
            Some(&bytes),
        )
        .unwrap_or_else(|e| panic!("{}: hash-object: {e}", self.name));
        Some(String::from_utf8_lossy(&id).trim().to_owned())
    }

    /// The id of content already in git's form.
    fn id_of(&self, content: &[u8]) -> String {
        let id = git_bytes(
            self.dir(),
            &[b"hash-object", b"--stdin", b"--no-filters"],
            Some(content),
        )
        .unwrap_or_else(|e| panic!("{}: hash-object: {e}", self.name));
        String::from_utf8_lossy(&id).trim().to_owned()
    }

    fn save(&self) -> Saved {
        let file = on_disk(&self.repo, &self.path);
        Saved {
            index: std::fs::read(self.dir().join(".git/index")).ok(),
            file: std::fs::read(&file).ok().map(|bytes| {
                let mode = std::fs::metadata(&file)
                    .map(|metadata| metadata.permissions().mode())
                    .unwrap_or(0o644);
                (bytes, mode)
            }),
        }
    }

    fn restore(&self, saved: &Saved) {
        let index = self.dir().join(".git/index");
        match &saved.index {
            Some(bytes) => std::fs::write(&index, bytes)
                .unwrap_or_else(|e| panic!("{}: restoring the index: {e}", self.name)),
            None => {
                let _ = std::fs::remove_file(&index);
            }
        }
        let file = on_disk(&self.repo, &self.path);
        match &saved.file {
            Some((bytes, mode)) => {
                write(&self.repo, &self.path, bytes);
                std::fs::set_permissions(&file, std::fs::Permissions::from_mode(*mode))
                    .unwrap_or_else(|e| panic!("{}: restoring the mode: {e}", self.name));
            }
            None => {
                let _ = std::fs::remove_file(&file);
            }
        }
    }

    /// The changed lines `git diff` (or `--cached`) prints for the path now: every removed
    /// line, then every added line, each in file order, by its bytes.
    fn changed_now(&self, action: PatchAction) -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
        let output = if self.untracked && action != PatchAction::Stage {
            git_diff_bytes(
                self.dir(),
                &[
                    b"diff",
                    b"--no-color",
                    b"--no-ext-diff",
                    b"--no-textconv",
                    b"--no-index",
                    b"--",
                    b"/dev/null",
                    &self.path,
                ],
            )
        } else {
            let mut args: Vec<&[u8]> = vec![
                b"--literal-pathspecs",
                b"diff",
                b"--no-color",
                b"--no-ext-diff",
                b"--no-textconv",
            ];
            match action {
                // A pair is read as git pairs it within its two paths: a rename or a copy,
                // and only its own record, not a copy source's modification beside it.
                PatchAction::Unstage if self.renamed_from.is_some() => {
                    args.extend([&b"--cached"[..], b"-C", b"--diff-filter=RC"]);
                }
                PatchAction::Unstage => args.extend([&b"--cached"[..], b"-M"]),
                PatchAction::Stage | PatchAction::Discard => args.push(b"--no-renames"),
            }
            args.push(b"--");
            args.push(&self.path);
            if action == PatchAction::Unstage
                && let Some(from) = &self.renamed_from
            {
                args.push(from);
            }
            git_diff_bytes(self.dir(), &args)
        };
        changed_lines(&output)
    }
}

struct Saved {
    index: Option<Vec<u8>>,
    file: Option<(Vec<u8>, u32)>,
}

/// The `-` and `+` lines of every hunk of a unified diff, read by the counts each hunk
/// header gives, so that a content line that looks like a header is read as content.
fn changed_lines(output: &[u8]) -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
    let mut removed = Vec::new();
    let mut added = Vec::new();
    let mut lines = output.split(|byte| *byte == b'\n').peekable();
    while let Some(line) = lines.next() {
        let Some(header) = line.strip_prefix(b"@@ -") else {
            continue;
        };
        let header = String::from_utf8_lossy(header);
        let count = |spec: &str| -> u32 {
            spec.split_once(',')
                .map_or(Ok(1), |(_, count)| count.parse())
                .unwrap_or_else(|e| panic!("a hunk header {header}: {e}"))
        };
        let mut specs = header.split(' ');
        let mut old = count(specs.next().unwrap_or_default());
        let mut new = count(specs.next().unwrap_or_default().trim_start_matches('+'));
        while old > 0 || new > 0 {
            let Some(body) = lines.next() else {
                panic!("a hunk ended early");
            };
            match body.first() {
                Some(b'-') => {
                    removed.push(body[1..].to_vec());
                    old -= 1;
                }
                Some(b'+') => {
                    added.push(body[1..].to_vec());
                    new -= 1;
                }
                Some(b'\\') => {}
                Some(_) | None => {
                    old = old.saturating_sub(1);
                    new = new.saturating_sub(1);
                }
            }
        }
        while lines.peek().is_some_and(|next| next.starts_with(b"\\")) {
            lines.next();
        }
    }
    (removed, added)
}

/// The forward rule, read straight off the changed ranges with no hunks or headers: an
/// unselected removal stays, a selected addition arrives after what stays of its change.
fn forward_rule(text: &TextDiff, selection: &Selection) -> Vec<u8> {
    let mut out: Vec<DiffLine> = Vec::new();
    let mut cursor = 0u32;
    for change in text.changes() {
        for index in cursor..change.removed.start().index() {
            out.extend(text.old_line(LineNumber::from_index(index)).cloned());
        }
        for line in change.removed.numbers() {
            if !selection.holds_removed(line) {
                out.extend(text.old_line(line).cloned());
            }
        }
        for line in change.added.numbers() {
            if selection.holds_added(line) {
                out.extend(text.new_line(line).cloned());
            }
        }
        cursor = change.removed.end().index();
    }
    for index in cursor..text.old_lines().len() as u32 {
        out.extend(text.old_line(LineNumber::from_index(index)).cloned());
    }
    bytes_of(&out)
}

/// git's mirrored rule for undoing a selection (E1b): a patch of the ORIGINAL diff, one hunk
/// over the whole file, in which a selected removal is `-`, an unselected removal is
/// dropped, a selected addition is `+` and an unselected addition is context — applied in
/// reverse to the new side. Written here from the format; nothing is inverted.
fn mirrored_rule(text: &TextDiff, selection: &Selection) -> Vec<u8> {
    let mut body = Vec::new();
    let (mut old_count, mut new_count) = (0u32, 0u32);
    let mut emit = |marker: u8, line: Option<&DiffLine>, old: bool, new: bool| {
        let Some(line) = line else { return };
        body.push(marker);
        body.extend_from_slice(line.bytes());
        body.push(b'\n');
        if !line.ends_with_newline() {
            body.extend_from_slice(b"\\ No newline at end of file\n");
        }
        old_count += u32::from(old);
        new_count += u32::from(new);
    };
    let mut cursor = 0u32;
    for change in text.changes() {
        for index in cursor..change.removed.start().index() {
            emit(
                b' ',
                text.old_line(LineNumber::from_index(index)),
                true,
                true,
            );
        }
        for line in change.removed.numbers() {
            if selection.holds_removed(line) {
                emit(b'-', text.old_line(line), true, false);
            }
        }
        for line in change.added.numbers() {
            if selection.holds_added(line) {
                emit(b'+', text.new_line(line), false, true);
            } else {
                emit(b' ', text.new_line(line), true, true);
            }
        }
        cursor = change.removed.end().index();
    }
    for index in cursor..text.old_lines().len() as u32 {
        emit(
            b' ',
            text.old_line(LineNumber::from_index(index)),
            true,
            true,
        );
    }
    let mut patch = format!(
        "@@ -{},{old_count} +{},{new_count} @@\n",
        u32::from(old_count > 0),
        u32::from(new_count > 0)
    )
    .into_bytes();
    patch.extend_from_slice(&body);
    let by_the_applier = apply_patch_in_reverse(&text.new_content(), &patch)
        .unwrap_or_else(|e| panic!("the mirrored patch did not apply in reverse: {e}"));
    let by_git = reversed_by_git(&text.new_content(), &patch);
    assert_eq!(
        by_the_applier, by_git,
        "the reference applier and `git apply -R` read the mirrored patch differently"
    );
    by_git
}

/// `hunk` applied by real `git apply -R` to a file holding `content`, in a scratch directory
/// that is no repository: E1b's mirrored patch, reversed by git itself.
fn reversed_by_git(content: &[u8], hunk: &[u8]) -> Vec<u8> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "cairn-mirrored-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("making {}: {e}", dir.display()));
    std::fs::write(dir.join("f"), content).unwrap_or_else(|e| panic!("writing: {e}"));
    let mut patch = b"--- a/f\n+++ b/f\n".to_vec();
    patch.extend_from_slice(hunk);
    let mut command = Command::new("git");
    command
        .current_dir(&dir)
        .args(["apply", "-R", "--whitespace=nowarn", "-"])
        .env("GIT_CEILING_DIRECTORIES", std::env::temp_dir())
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("HOME", empty_home())
        .env("XDG_CONFIG_HOME", empty_home())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .unwrap_or_else(|e| panic!("could not start git apply -R: {e}"));
    if let Some(mut pipe) = child.stdin.take() {
        use std::io::Write;
        pipe.write_all(&patch)
            .unwrap_or_else(|e| panic!("writing the mirrored patch: {e}"));
    }
    let output = child
        .wait_with_output()
        .unwrap_or_else(|e| panic!("waiting for git apply -R: {e}"));
    assert!(
        output.status.success(),
        "git apply -R refused the mirrored patch: {}\n{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&patch)
    );
    let result = std::fs::read(dir.join("f")).unwrap_or_else(|e| panic!("reading: {e}"));
    let _ = std::fs::remove_dir_all(&dir);
    result
}

fn bytes_of(lines: &[DiffLine]) -> Vec<u8> {
    let mut out = Vec::new();
    for line in lines {
        out.extend_from_slice(line.bytes());
        if line.ends_with_newline() {
            out.push(b'\n');
        }
    }
    out
}

/// The lines a selection leaves out, by side, in file order: what `git diff` must still
/// show afterwards.
fn left_out(text: &TextDiff, selection: &Selection) -> (Vec<Vec<u8>>, Vec<Vec<u8>>) {
    let mut removed = Vec::new();
    let mut added = Vec::new();
    for change in text.changes() {
        for line in change.removed.numbers() {
            if !selection.holds_removed(line) {
                removed.extend(text.old_line(line).map(|line| line.bytes().to_vec()));
            }
        }
        for line in change.added.numbers() {
            if !selection.holds_added(line) {
                added.extend(text.new_line(line).map(|line| line.bytes().to_vec()));
            }
        }
    }
    (removed, added)
}

/// A fixed-seed generator, so a failure names a seed that reproduces it.
struct Seeded(u64);

impl Seeded {
    fn coin(&mut self) -> bool {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33).is_multiple_of(2)
    }
}

/// Selections of lines alone: every line, only additions, only removals, the first and the
/// last line of every change, every other changed line, and seeded ones.
fn selections(text: &TextDiff) -> Vec<(String, Selection)> {
    let mut named = vec![("every line".to_owned(), Selection::with_every_change(text))];
    let mut additions = Selection::empty();
    let mut removals = Selection::empty();
    let mut first = Selection::empty();
    let mut last = Selection::empty();
    let mut alternate = Selection::empty();
    let mut flip = false;
    for change in text.changes() {
        for line in change.removed.numbers() {
            removals.select_removed(line);
            flip = !flip;
            if flip {
                alternate.select_removed(line);
            }
        }
        for line in change.added.numbers() {
            additions.select_added(line);
            flip = !flip;
            if flip {
                alternate.select_added(line);
            }
        }
        match (
            change.removed.numbers().next(),
            change.added.numbers().next(),
        ) {
            (Some(line), _) => first.select_removed(line),
            (None, Some(line)) => first.select_added(line),
            (None, None) => {}
        }
        match (
            change.added.numbers().last(),
            change.removed.numbers().last(),
        ) {
            (Some(line), _) => last.select_added(line),
            (None, Some(line)) => last.select_removed(line),
            (None, None) => {}
        }
    }
    named.push(("only additions".to_owned(), additions));
    named.push(("only removals".to_owned(), removals));
    named.push(("the first line of every change".to_owned(), first));
    named.push(("the last line of every change".to_owned(), last));
    named.push(("every other changed line".to_owned(), alternate));
    for seed in 1..=3u64 {
        let mut random = Seeded(seed);
        let mut selection = Selection::empty();
        for change in text.changes() {
            for line in change.removed.numbers() {
                if random.coin() {
                    selection.select_removed(line);
                }
            }
            for line in change.added.numbers() {
                if random.coin() {
                    selection.select_added(line);
                }
            }
        }
        named.push((format!("seed {seed}"), selection));
    }
    named
}

/// Runs every action of `case` over every selection, against the three oracles. Returns how
/// many applies were checked.
fn run(case: &Case) -> usize {
    let saved = case.save();
    let mut checked = 0;
    for &action in &case.actions {
        case.restore(&saved);
        let drawn = case.drawn(action);
        let DiffContent::Text { text, .. } = &drawn.content else {
            panic!(
                "{}: {action:?} drew no text: {:?}",
                case.name, drawn.content
            );
        };
        assert!(
            Selection::with_every_change(text).len() >= 4,
            "{}: the {action:?} diff has too few changed lines to select part of",
            case.name
        );
        let target = drawn.file.new_path.as_bytes().to_vec();
        for (name, selection) in selections(text) {
            let at = format!("{}, {action:?} {name}", case.name);
            if selection.is_empty() {
                continue;
            }
            case.restore(&saved);
            let patch = action_patch(action, &drawn, &selection);
            assert!(!patch.is_empty(), "{at}: no patch for a selection of lines");
            let staged_before = case.staged(&target);
            let file_before = std::fs::read(on_disk(&case.repo, &case.path)).ok();

            let apply: &[&[u8]] = match action {
                PatchAction::Discard => &[b"apply", b"--whitespace=nowarn", b"-"],
                PatchAction::Stage | PatchAction::Unstage => {
                    &[b"apply", b"--cached", b"--whitespace=nowarn", b"-"]
                }
            };
            if let Err(refused) = git_bytes(case.dir(), apply, Some(patch.as_bytes())) {
                panic!("{at}: git apply refused it: {refused}\n{patch:?}");
            }

            // Whether the file goes, decided from the case and never from the patch under
            // test: undoing every line of an added file, and nothing else, takes it away.
            let deletes = drawn.file.status == ChangeStatus::Added
                && action != PatchAction::Stage
                && selection.holds_every_change(text);
            assert_eq!(
                patch.text().contains("\ndeleted file mode "),
                deletes,
                "{at}: the patch {} the file\n{patch:?}",
                if deletes {
                    "does not delete"
                } else {
                    "deletes"
                }
            );
            let (side, derived) = match action {
                PatchAction::Stage => (text.old_content(), forward_rule(text, &selection)),
                PatchAction::Unstage | PatchAction::Discard => {
                    (text.new_content(), mirrored_rule(text, &selection))
                }
            };
            let applied = apply_patch(&side, patch.as_bytes())
                .unwrap_or_else(|e| panic!("{at}: the reference applier refused it: {e}"));
            assert_eq!(
                applied,
                derived,
                "{at}: the reference applier and the {} rule disagree\n{patch:?}",
                if action == PatchAction::Stage {
                    "forward"
                } else {
                    "mirrored"
                }
            );

            match action {
                PatchAction::Stage | PatchAction::Unstage => {
                    let now = case.staged(&target);
                    if deletes {
                        assert_eq!(now, None, "{at}: the entry was not removed");
                    } else {
                        let (_, id) =
                            now.unwrap_or_else(|| panic!("{at}: nothing is staged at the path"));
                        assert_eq!(
                            case.blob(&id),
                            derived,
                            "{at}: git staged other bytes than the oracles give\n{patch:?}"
                        );
                    }
                    assert_eq!(
                        std::fs::read(on_disk(&case.repo, &case.path)).ok(),
                        file_before,
                        "{at}: the working tree was touched"
                    );
                }
                PatchAction::Discard => {
                    let now = case.work_tree_id();
                    if deletes {
                        assert_eq!(now, None, "{at}: the file was not removed");
                    } else {
                        assert_eq!(
                            now,
                            Some(case.id_of(&derived)),
                            "{at}: git left another file than the oracles give\n{patch:?}"
                        );
                    }
                    assert_eq!(
                        case.staged(&target),
                        staged_before,
                        "{at}: the index was touched"
                    );
                }
            }

            if !deletes {
                assert_eq!(
                    case.changed_now(action),
                    left_out(text, &selection),
                    "{at}: git diff afterwards is not exactly what the selection left out"
                );
            }
            checked += 1;
        }
    }
    case.restore(&saved);
    checked
}

/// Builds a case whose path has staged and unstaged edits: `HEAD`'s lines, the index's and
/// the working tree's, each ended by `ending`.
fn edited(name: &str, path: &[u8], ending: &str, setup: impl FnOnce(&Repo)) -> Case {
    let case = Case::new(name, path);
    setup(&case.repo);
    write(&case.repo, path, &joined(&head_lines(), ending));
    case.repo.commit("base");
    write(&case.repo, path, &joined(&index_lines(), ending));
    case.repo.git(&["add", "--all", "."]);
    write(&case.repo, path, &joined(&work_lines(), ending));
    case
}

fn check(case: &Case) {
    let checked = run(case);
    assert!(
        checked >= 6 * case.actions.len(),
        "{}: only {checked} applies were checked",
        case.name
    );
}

/// C3: a modified file with staged and unstaged edits.
#[test]
fn lines_of_a_modification_stage_unstage_and_discard_as_git_says() {
    check(&edited("modification", b"file.txt", "\n", |_| {}));
}

/// C3: an untracked file, staged as a new file of the selected lines (E3) and discarded as
/// a partial deletion written as a modification (E4).
#[test]
fn lines_of_an_untracked_file_stage_and_discard_as_git_says() {
    let mut case = Case::new("untracked", b"new.txt");
    write(&case.repo, b"base.txt", b"base\n");
    case.repo.commit("base");
    write(&case.repo, b"new.txt", &joined(&work_lines(), "\n"));
    case.untracked = true;
    case.actions = vec![PatchAction::Stage, PatchAction::Discard];
    check(&case);
}

/// C3: an intent-to-add entry, whose unstaged diff is a new file — so its stage is a `new
/// file mode` patch applied over the intent-to-add entry, which is state.md's open question,
/// answered on every git here (see also `a_new_file_patch_applies_over_an_intent_to_add_entry`).
#[test]
fn lines_of_an_intent_to_add_file_stage_and_discard_as_git_says() {
    let mut case = Case::new("intent to add", b"new.txt");
    write(&case.repo, b"base.txt", b"base\n");
    case.repo.commit("base");
    write(&case.repo, b"new.txt", &joined(&work_lines(), "\n"));
    case.repo.git(&["add", "-N", "new.txt"]);
    case.actions = vec![PatchAction::Stage, PatchAction::Discard];
    check(&case);
}

/// C3: CRLF in the working tree under `core.autocrlf=true`, the diff and every patch in
/// git's form (LF), the working tree after a discard compared in git's form and kept CRLF.
#[test]
fn lines_of_a_crlf_file_under_autocrlf_stage_unstage_and_discard_as_git_says() {
    let case = edited("crlf", b"dos.txt", "\r\n", |repo| {
        repo.config("core.autocrlf", "true");
        repo.config("core.safecrlf", "false");
    });
    check(&case);
    // What a discard leaves on disk keeps the working tree's endings.
    let saved = case.save();
    let drawn = case.drawn(PatchAction::Discard);
    let DiffContent::Text { text, .. } = &drawn.content else {
        panic!("{:?}", drawn.content);
    };
    let mut one = Selection::empty();
    one.select_added(
        text.changes()
            .iter()
            .find_map(|change| change.added.numbers().next())
            .unwrap_or_else(|| panic!("no added line")),
    );
    let patch = action_patch(PatchAction::Discard, &drawn, &one);
    git_bytes(
        case.dir(),
        &[b"apply", b"--whitespace=nowarn", b"-"],
        Some(patch.as_bytes()),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let on_disk = std::fs::read(on_disk(&case.repo, b"dos.txt")).unwrap_or_default();
    assert!(
        on_disk
            .windows(2)
            .filter(|pair| pair[1] == b'\n')
            .all(|pair| pair[0] == b'\r'),
        "a discard left a line without its CR: {:?}",
        String::from_utf8_lossy(&on_disk)
    );
    case.restore(&saved);
}

/// C3: a path under a clean filter driver (rot13 both ways): the diff is in the clean form,
/// the index holds it, and a discard runs the filter both ways through `git apply`.
#[test]
fn lines_of_a_filtered_file_stage_unstage_and_discard_as_git_says() {
    let case = edited("clean filter", b"secret.txt", "\n", |repo| {
        repo.config("filter.rot13.clean", "tr A-Za-z N-ZA-Mn-za-m");
        repo.config("filter.rot13.smudge", "tr A-Za-z N-ZA-Mn-za-m");
        repo.config("filter.rot13.required", "true");
        repo.write(".gitattributes", b"secret.txt filter=rot13\n");
    });
    // The index holds the clean form, so the filter is really in play.
    let (_, id) = case
        .staged(b"secret.txt")
        .unwrap_or_else(|| panic!("nothing staged"));
    assert!(
        String::from_utf8_lossy(&case.blob(&id)).contains("yvar 00"),
        "the clean filter did not run"
    );
    check(&case);
}

/// C3: an unborn branch, where the staged diff is against the empty tree — every staged
/// line an addition, unstaged as a partial deletion of the index entry.
#[test]
fn lines_on_an_unborn_branch_stage_unstage_and_discard_as_git_says() {
    let case = Case::new("unborn", b"first.txt");
    write(&case.repo, b"first.txt", &joined(&index_lines(), "\n"));
    case.repo.git(&["add", "--all", "."]);
    write(&case.repo, b"first.txt", &joined(&work_lines(), "\n"));
    assert!(
        case.repo
            .try_git(&["rev-parse", "--verify", "-q", "HEAD"], &[], None)
            .is_err(),
        "the branch is not unborn"
    );
    // The staged diff is one addition; give unstage something to select part of.
    let saved = case.save();
    let drawn = case.drawn(PatchAction::Unstage);
    assert_eq!(drawn.file.status, ChangeStatus::Added);
    let DiffContent::Text { text, .. } = &drawn.content else {
        panic!("{:?}", drawn.content);
    };
    let mut checked = 0;
    for count in [1u32, 5, 12] {
        let mut selection = Selection::empty();
        for line in text.changes()[0].added.numbers().take(count as usize) {
            selection.select_added(line);
        }
        case.restore(&saved);
        let patch = action_patch(PatchAction::Unstage, &drawn, &selection);
        git_bytes(
            case.dir(),
            &[b"apply", b"--cached", b"--whitespace=nowarn", b"-"],
            Some(patch.as_bytes()),
        )
        .unwrap_or_else(|e| panic!("unborn, unstage {count}: {e}\n{patch:?}"));
        let (_, id) = case
            .staged(b"first.txt")
            .unwrap_or_else(|| panic!("unborn, unstage {count}: the entry went"));
        let derived = mirrored_rule(text, &selection);
        assert_eq!(case.blob(&id), derived, "unborn, unstage {count}");
        assert_eq!(
            apply_patch(&text.new_content(), patch.as_bytes()).as_deref(),
            Ok(&derived[..])
        );
        assert_eq!(
            case.changed_now(PatchAction::Unstage),
            left_out(text, &selection),
            "unborn, unstage {count}: git diff --cached afterwards"
        );
        checked += 1;
    }
    // Every line unstaged on an unborn branch takes the entry away, as the whole of an
    // addition undone does.
    case.restore(&saved);
    let every = Selection::with_every_change(text);
    let patch = action_patch(PatchAction::Unstage, &drawn, &every);
    git_bytes(
        case.dir(),
        &[b"apply", b"--cached", b"--whitespace=nowarn", b"-"],
        Some(patch.as_bytes()),
    )
    .unwrap_or_else(|e| panic!("unborn, unstage every line: {e}\n{patch:?}"));
    assert_eq!(
        case.staged(b"first.txt"),
        None,
        "unborn, unstage every line: the entry stayed"
    );
    assert!(
        on_disk(&case.repo, b"first.txt").exists(),
        "unborn, unstage every line: the working tree lost the file"
    );
    checked += 1;
    case.restore(&saved);
    assert_eq!(checked, 4);
    let mut case = case;
    case.actions = vec![PatchAction::Stage, PatchAction::Discard];
    check(&case);
}

/// C3 beside C4: a file whose mode changed on both sides as well as its lines, each line
/// selection carrying no mode (C4 reads the modes back).
#[test]
fn lines_beside_a_mode_change_stage_unstage_and_discard_as_git_says() {
    let case = edited("mode change", b"run.sh", "\n", |_| {});
    // Staged: 644 -> 755 with the staged edits; unstaged: 755 -> 644 with the working
    // tree's.
    let saved_work = std::fs::read(on_disk(&case.repo, b"run.sh")).unwrap_or_default();
    write(&case.repo, b"run.sh", &joined(&index_lines(), "\n"));
    chmod(&case.repo, b"run.sh", 0o755);
    case.repo.git(&["add", "run.sh"]);
    write(&case.repo, b"run.sh", &saved_work);
    chmod(&case.repo, b"run.sh", 0o644);
    for action in [PatchAction::Stage, PatchAction::Unstage] {
        assert!(
            case.drawn(action).file.mode_changed(),
            "the {action:?} diff lost its mode change"
        );
    }
    check(&case);
}

/// C3 and R2.6: a staged rename with edits, drawn as `git diff --cached` pairs it, its lines
/// unstaged as content at the new path; the unstaged edits on top staged and discarded.
#[test]
fn lines_of_a_staged_rename_unstage_as_content_at_its_new_path() {
    let mut case = Case::new("staged rename", b"new.txt");
    write(&case.repo, b"old.txt", &joined(&head_lines(), "\n"));
    case.repo.commit("base");
    case.repo.git(&["mv", "old.txt", "new.txt"]);
    write(&case.repo, b"new.txt", &joined(&index_lines(), "\n"));
    case.repo.git(&["add", "new.txt"]);
    write(&case.repo, b"new.txt", &joined(&work_lines(), "\n"));
    case.renamed_from = Some(b"old.txt".to_vec());
    let staged = case.drawn(PatchAction::Unstage);
    assert!(
        staged.file.is_rename(),
        "the staged diff is not the rename git pairs: {:?}",
        staged.file
    );
    let patch = action_patch(
        PatchAction::Unstage,
        &staged,
        &Selection::with_every_change(staged.text().unwrap_or_else(|| panic!("no text"))),
    );
    assert!(
        patch.text().starts_with("diff --git a/new.txt b/new.txt\n")
            && !patch.text().contains("rename"),
        "unstaging lines of a rename moved the path:\n{patch:?}"
    );
    check(&case);
    // The rename itself is still staged afterwards: the source is gone from the index.
    assert_eq!(case.staged(b"old.txt"), None);
}

/// C3 and R2.6 for a copy: a staged copy of an edited source, drawn as `git diff --cached`
/// pairs it under `diff.renames=copies`, its lines unstaged as content at the copy's path
/// with real git — the one place `ChangedFile::inverted`'s copy arm reaches `git apply`.
#[test]
fn lines_of_a_staged_copy_unstage_as_content_at_its_path() {
    let mut case = Case::new("staged copy", b"copy.txt");
    case.repo.config("diff.renames", "copies");
    write(&case.repo, b"src.txt", &joined(&head_lines(), "\n"));
    case.repo.commit("base");
    write(&case.repo, b"copy.txt", &joined(&index_lines(), "\n"));
    let mut source = head_lines();
    source.push("the source moved on".to_owned());
    write(&case.repo, b"src.txt", &joined(&source, "\n"));
    case.repo.git(&["add", "copy.txt", "src.txt"]);
    write(&case.repo, b"copy.txt", &joined(&work_lines(), "\n"));
    case.renamed_from = Some(b"src.txt".to_vec());
    let staged = case.drawn(PatchAction::Unstage);
    assert!(
        staged.file.is_copy(),
        "the staged diff is not the copy git pairs: {:?}",
        staged.file
    );
    let patch = action_patch(
        PatchAction::Unstage,
        &staged,
        &Selection::with_every_change(staged.text().unwrap_or_else(|| panic!("no text"))),
    );
    assert!(
        patch
            .text()
            .starts_with("diff --git a/copy.txt b/copy.txt\n")
            && !patch.text().contains("copy from")
            && !patch.text().contains("deleted file"),
        "unstaging lines of a copy did not change content at its path:\n{patch:?}"
    );
    check(&case);
    // The source is untouched by any of it.
    let (_, id) = case
        .staged(b"src.txt")
        .unwrap_or_else(|| panic!("the source left the index"));
    assert_eq!(case.blob(&id), joined(&source, "\n"));
}

/// C3: each awkward name, modified, through every action: the path lines are C-quoted as
/// git quotes them (R2.5), and `git apply` finds the file each names.
#[test]
fn lines_of_awkwardly_named_files_stage_unstage_and_discard_as_git_says() {
    for (name, path) in [
        ("a space", &b"sp ace.txt"[..]),
        ("a tab", b"ta\tb.txt"),
        ("a quote", b"quo\"te.txt"),
        ("a backslash", b"back\\slash.txt"),
        ("a newline", b"new\nline.txt"),
        ("a control byte", b"ctl\x01.txt"),
        ("invalid UTF-8", b"bad\xff.txt"),
        ("a carriage return", b"cr\r.txt"),
        ("a delete byte", b"del\x7f.txt"),
        ("a bell", b"bell\x07.txt"),
        ("a backspace", b"bs\x08.txt"),
        ("a vertical tab", b"vt\x0b.txt"),
        ("a form feed", b"ff\x0c.txt"),
    ] {
        let case = edited(name, path, "\n", |_| {});
        // The emitted headers are git's own, byte for byte.
        let drawn = case.drawn(PatchAction::Stage);
        let ours = action_patch(
            PatchAction::Stage,
            &drawn,
            &Selection::with_every_change(drawn.text().unwrap_or_else(|| panic!("no text"))),
        );
        let theirs = git_diff_bytes(
            case.dir(),
            &[
                b"--literal-pathspecs",
                b"diff",
                b"--no-color",
                b"--no-ext-diff",
                b"--",
                path,
            ],
        );
        let header = |patch: &[u8]| -> Vec<Vec<u8>> {
            patch
                .split(|byte| *byte == b'\n')
                .filter(|line| {
                    line.starts_with(b"diff --git ")
                        || line.starts_with(b"--- ")
                        || line.starts_with(b"+++ ")
                })
                .map(<[u8]>::to_vec)
                .collect()
        };
        assert_eq!(
            header(ours.as_bytes()),
            header(&theirs),
            "{name}: the path lines are not git's"
        );
        check(&case);
    }
}

/// state.md's open question, settled: a `new file mode` patch of part of a file applies over
/// an intent-to-add entry, on every git, and leaves a real entry holding the lines selected.
#[test]
fn a_new_file_patch_applies_over_an_intent_to_add_entry() {
    let case = Case::new("ita new file", b"n.txt");
    write(&case.repo, b"base.txt", b"base\n");
    case.repo.commit("base");
    write(&case.repo, b"n.txt", b"one\ntwo\nthree\n");
    case.repo.git(&["add", "-N", "n.txt"]);
    let drawn = case.drawn(PatchAction::Stage);
    assert_eq!(drawn.file.status, ChangeStatus::Added);
    let mut part = Selection::empty();
    part.select_added(LineNumber::from_index(0));
    part.select_added(LineNumber::from_index(2));
    let patch = action_patch(PatchAction::Stage, &drawn, &part);
    assert!(patch.text().contains("new file mode 100644\n"), "{patch:?}");
    git_bytes(
        case.dir(),
        &[b"apply", b"--cached", b"--whitespace=nowarn", b"-"],
        Some(patch.as_bytes()),
    )
    .unwrap_or_else(|e| panic!("a new-file patch was refused over intent-to-add: {e}"));
    let (_, id) = case
        .staged(b"n.txt")
        .unwrap_or_else(|| panic!("nothing staged"));
    assert_eq!(case.blob(&id), b"one\nthree\n");
    let status = case.repo.git(&["status", "--porcelain=v2", "--", "n.txt"]);
    assert!(
        status.starts_with("1 AM "),
        "the entry is not a staged addition with the rest unstaged: {status}"
    );
}

/// C4: on a file whose mode changed beside its lines, a selection of lines stages,
/// unstages and discards no mode, and the mode alone moves the mode alone — the modes read
/// back from git's index and from the file.
#[test]
fn a_selection_of_lines_moves_no_mode_and_the_mode_moves_alone() {
    let case = edited("c4", b"tool.sh", "\n", |_| {});
    let work = std::fs::read(on_disk(&case.repo, b"tool.sh")).unwrap_or_default();
    write(&case.repo, b"tool.sh", &joined(&index_lines(), "\n"));
    chmod(&case.repo, b"tool.sh", 0o755);
    case.repo.git(&["add", "tool.sh"]);
    write(&case.repo, b"tool.sh", &work);
    chmod(&case.repo, b"tool.sh", 0o644);
    let saved = case.save();
    let executable = |case: &Case| -> bool {
        std::fs::metadata(on_disk(&case.repo, b"tool.sh"))
            .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    };

    let mut mode_only = Selection::empty();
    mode_only.select_mode();
    for action in [
        PatchAction::Stage,
        PatchAction::Unstage,
        PatchAction::Discard,
    ] {
        case.restore(&saved);
        let drawn = case.drawn(action);
        let text = drawn.text().unwrap_or_else(|| panic!("no text")).clone();
        assert!(
            drawn.file.mode_changed(),
            "{action:?}: no mode change drawn"
        );
        let lines = Selection::with_every_change(&text);
        for (what, selection) in [("lines", &lines), ("mode", &mode_only)] {
            case.restore(&saved);
            let (mode_before, id_before) = case
                .staged(b"tool.sh")
                .unwrap_or_else(|| panic!("nothing staged"));
            let exec_before = executable(&case);
            let work_before = case.work_tree_id();
            let patch = action_patch(action, &drawn, selection);
            assert_eq!(
                patch.text().contains("old mode "),
                what == "mode",
                "{action:?} {what}: the mode lines\n{patch:?}"
            );
            let apply: &[&[u8]] = match action {
                PatchAction::Discard => &[b"apply", b"--whitespace=nowarn", b"-"],
                PatchAction::Stage | PatchAction::Unstage => {
                    &[b"apply", b"--cached", b"--whitespace=nowarn", b"-"]
                }
            };
            git_bytes(case.dir(), apply, Some(patch.as_bytes()))
                .unwrap_or_else(|e| panic!("{action:?} {what}: {e}\n{patch:?}"));
            let (mode_after, id_after) = case
                .staged(b"tool.sh")
                .unwrap_or_else(|| panic!("nothing staged"));
            match (action, what) {
                (PatchAction::Stage | PatchAction::Unstage, "lines") => {
                    assert_eq!(mode_after, mode_before, "{action:?}: lines moved the mode");
                    assert_ne!(id_after, id_before, "{action:?}: lines moved no line");
                }
                (PatchAction::Stage | PatchAction::Unstage, _) => {
                    assert_ne!(mode_after, mode_before, "{action:?}: the mode did not move");
                    assert_eq!(id_after, id_before, "{action:?}: the mode moved a line");
                    let wanted = drawn
                        .file
                        .inverted()
                        .new_mode
                        .filter(|_| action == PatchAction::Unstage)
                        .or(drawn.file.new_mode.filter(|_| action == PatchAction::Stage))
                        .map(|mode| mode.octal().to_owned());
                    assert_eq!(Some(mode_after), wanted, "{action:?}: the wrong mode");
                }
                (PatchAction::Discard, "lines") => {
                    assert_eq!(
                        executable(&case),
                        exec_before,
                        "discarding lines moved the mode"
                    );
                    assert_ne!(case.work_tree_id(), work_before, "no line was discarded");
                    assert_eq!((mode_after, id_after), (mode_before, id_before));
                }
                (PatchAction::Discard, _) => {
                    assert_ne!(
                        executable(&case),
                        exec_before,
                        "discarding the mode did not"
                    );
                    assert_eq!(
                        case.work_tree_id(),
                        work_before,
                        "the mode discarded a line"
                    );
                    assert_eq!((mode_after, id_after), (mode_before, id_before));
                }
            }
        }
    }
    case.restore(&saved);
}
