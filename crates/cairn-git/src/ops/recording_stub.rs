//! A `git` that records what it was given — its arguments, its environment, its stdin — and
//! then runs the machine's real `git` with exactly that, for the write verbs' argv tests
//! (staging-and-commit C9). The runner is crate-private, so these tests live in the crate;
//! the effect of each verb is pinned again against real git alone in
//! `crates/cairn-git/tests/diff/write_verbs.rs`.
//!
//! Recording before running is what lets one test both read the exact `argv` a verb built
//! and have the verb's own reads (`hash-object`) answered truly, so a discard runs its
//! re-check against the real repository the fixture built.

use std::collections::BTreeMap;
use std::path::PathBuf;

use cairn_model::{FileDiff, RepoPath, Selection};

use super::{Askpass, GitBinary, GitEnvironment, WriteAuthority};
use crate::process::stub_git::{StubGit, discover_retrying};
use crate::{CancelSignal, ContentOptions, Repository, WorkingTreeDiff};

/// One invocation the stub saw.
#[derive(Debug, Clone)]
pub(crate) struct Recorded {
    pub(crate) arguments: Vec<String>,
    pub(crate) environment: BTreeMap<String, String>,
    pub(crate) stdin: Vec<u8>,
}

impl Recorded {
    /// The arguments after the repository's location (`--git-dir=`, `--work-tree=`), which
    /// every invocation in a repository carries first (`crate::process`).
    pub(crate) fn arguments_after_location(&self) -> Vec<String> {
        self.arguments
            .iter()
            .skip_while(|argument| {
                argument.starts_with("--git-dir=") || argument.starts_with("--work-tree=")
            })
            .cloned()
            .collect()
    }

    /// A write's environment: the pins every invocation has, and none of a read's — and no
    /// `GIT_LITERAL_PATHSPECS`, which is git's global option on argv instead (C9).
    pub(crate) fn assert_a_write(&self) {
        let get = |name: &str| self.environment.get(name).map(String::as_str);
        assert_eq!(get("GIT_TERMINAL_PROMPT"), Some("0"), "{self:?}");
        assert_eq!(get("GIT_EDITOR"), Some("false"), "{self:?}");
        for absent in [
            "GIT_OPTIONAL_LOCKS",
            "GIT_NO_LAZY_FETCH",
            "GIT_LITERAL_PATHSPECS",
        ] {
            assert_eq!(get(absent), None, "a write carried {absent}: {self:?}");
        }
    }

    /// A read's environment: its two pins, and no `GIT_LITERAL_PATHSPECS`.
    pub(crate) fn assert_a_read(&self) {
        let get = |name: &str| self.environment.get(name).map(String::as_str);
        assert_eq!(get("GIT_OPTIONAL_LOCKS"), Some("0"), "{self:?}");
        assert_eq!(get("GIT_NO_LAZY_FETCH"), Some("1"), "{self:?}");
        assert_eq!(get("GIT_LITERAL_PATHSPECS"), None, "{self:?}");
    }
}

/// A repository built by real git, the recording `git`, and the real one beside it.
pub(crate) struct RecordingStub {
    stub: StubGit,
    real: GitBinary,
    root: PathBuf,
}

impl RecordingStub {
    /// `file.txt` committed with twenty lines, a staged edit on top and an unstaged edit on
    /// that, and an untracked `new.txt`.
    pub(crate) fn new() -> Self {
        let program = GitBinary::discover(&Askpass::new(StubGit::HELPER, None))
            .unwrap_or_else(|error| panic!("the machine's git: {error}"))
            .path()
            .to_owned();
        let stub = StubGit::with_git_from(|directory| {
            let records = directory.join("records");
            format!(
                "if [ \"$1\" = --version ]; then exec '{git}' --version; fi\n\
                 /usr/bin/mkdir -p '{records}'\n\
                 n=0; while [ -e \"$(printf '%s/%06d' '{records}' \"$n\")\" ]; do n=$((n+1)); done\n\
                 d=$(printf '%s/%06d' '{records}' \"$n\")\n\
                 /usr/bin/mkdir \"$d\"\n\
                 for a in \"$@\"; do printf '%s\\0' \"$a\"; done > \"$d/arguments\"\n\
                 /usr/bin/env -0 > \"$d/environment\"\n\
                 /usr/bin/cat > \"$d/stdin\"\n\
                 exec '{git}' \"$@\" < \"$d/stdin\"",
                git = program.display(),
                records = records.display(),
            )
        });
        let root = stub.directory().join("repository");
        std::fs::create_dir_all(&root)
            .unwrap_or_else(|error| panic!("making {}: {error}", root.display()));
        // The real git, given nothing of this machine's environment but `PATH`, and the
        // fixture as its home: no configuration of the user's reaches the fixture.
        let home = root.clone();
        let real = GitBinary::discover_with(GitEnvironment::new(
            move |name| match name {
                "PATH" => std::env::var_os("PATH"),
                "HOME" => Some(home.clone().into_os_string()),
                _ => None,
            },
            &Askpass::new(StubGit::HELPER, None),
        ))
        .unwrap_or_else(|error| panic!("the machine's git: {error}"));
        let fixture = Self { stub, real, root };
        fixture.git(&["init", "-q", "."]);
        let lines = |edit: &dyn Fn(usize) -> String| -> String {
            (0..20).map(|n| format!("{}\n", edit(n))).collect()
        };
        fixture.write("file.txt", &lines(&|n| format!("line {n}")));
        fixture.git(&["add", "file.txt"]);
        fixture.git(&["commit", "-qm", "base"]);
        fixture.write(
            "file.txt",
            &lines(&|n| {
                if n == 3 {
                    "staged 3".into()
                } else {
                    format!("line {n}")
                }
            }),
        );
        fixture.git(&["add", "file.txt"]);
        fixture.write(
            "file.txt",
            &lines(&|n| match n {
                3 => "staged 3".into(),
                12 => "work 12".into(),
                _ => format!("line {n}"),
            }),
        );
        fixture.write("new.txt", "one\ntwo\n");
        fixture
    }

    /// Runs the real git in the fixture, as a write through the crate's own runner.
    fn git(&self, args: &[&str]) {
        self.real
            .write_invocation(WriteAuthority::new())
            .arg("-C")
            .arg(&self.root)
            .args([
                "-c",
                "user.name=A",
                "-c",
                "user.email=a@example.com",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .collected()
            .unwrap_or_else(|error| panic!("git {args:?}: {error}"));
    }

    pub(crate) fn write(&self, path: &str, content: &str) {
        std::fs::write(self.root.join(path), content)
            .unwrap_or_else(|error| panic!("writing {path}: {error}"));
    }

    /// The recording `git`, discovered as the application discovers one.
    pub(crate) fn git_binary(&self) -> GitBinary {
        discover_retrying(self.stub.environment())
            .unwrap_or_else(|error| panic!("the recording git: {error}"))
    }

    pub(crate) fn repository(&self) -> Repository {
        Repository::discover(&self.root).unwrap_or_else(|error| panic!("the fixture: {error}"))
    }

    /// `path`'s diff on `which` side, read by the real git.
    pub(crate) fn diff(&self, path: &str, which: WorkingTreeDiff) -> FileDiff {
        self.repository()
            .working_tree_diff(
                &self.real,
                &RepoPath::new(path),
                which,
                &ContentOptions::default(),
                &CancelSignal::new(),
            )
            .unwrap_or_else(|error| panic!("{path}'s {which:?} diff: {error}"))
            .unwrap_or_else(|| panic!("{path} has no {which:?} diff"))
    }

    /// `diff`'s first change, selected whole.
    pub(crate) fn first_change(diff: &FileDiff) -> Selection {
        let text = diff
            .text()
            .unwrap_or_else(|| panic!("{} drew no text", diff.file.new_path));
        let mut selection = Selection::empty();
        selection.select_change(&text.changes()[0]);
        selection
    }

    /// Every invocation recorded so far, in the order they ran.
    pub(crate) fn recorded(&self) -> Vec<Recorded> {
        let records = self.stub.directory().join("records");
        let Ok(entries) = std::fs::read_dir(&records) else {
            return Vec::new();
        };
        let mut directories: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
        directories.sort();
        directories
            .iter()
            .map(|directory| {
                let fields = |name: &str| -> Vec<String> {
                    let bytes = std::fs::read(directory.join(name))
                        .unwrap_or_else(|error| panic!("reading a record: {error}"));
                    bytes
                        .split(|byte| *byte == 0)
                        .filter(|field| !field.is_empty())
                        .map(|field| String::from_utf8_lossy(field).into_owned())
                        .collect()
                };
                Recorded {
                    arguments: fields("arguments"),
                    environment: fields("environment")
                        .iter()
                        .filter_map(|line| line.split_once('='))
                        .map(|(name, value)| (name.to_owned(), value.to_owned()))
                        .collect(),
                    stdin: std::fs::read(directory.join("stdin"))
                        .unwrap_or_else(|error| panic!("reading a record: {error}")),
                }
            })
            .collect()
    }

    /// Forgets what was recorded, so a test reads only what it ran next.
    pub(crate) fn forget(&self) {
        let _ = std::fs::remove_dir_all(self.stub.directory().join("records"));
    }
}
