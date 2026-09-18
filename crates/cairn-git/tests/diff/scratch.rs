//! A scratch index and object directory over a repository, so that applying a patch with
//! real `git` writes nothing into the repository it applies to.
//!
//! `GIT_INDEX_FILE` puts the index in a temporary directory, `GIT_OBJECT_DIRECTORY` puts
//! every object `apply` and `write-tree` create there too, and
//! `GIT_ALTERNATE_OBJECT_DIRECTORIES` lets those commands still read the repository's own
//! objects. Nothing under the subject's `.git` is written.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug)]
pub struct Scratch {
    repo: PathBuf,
    dir: PathBuf,
}

impl Scratch {
    pub fn over(repo: &Path) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let unique = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("cairn-scratch-{}-{unique}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("objects"))
            .unwrap_or_else(|e| panic!("making {}: {e}", dir.display()));
        Self {
            repo: repo.to_owned(),
            dir,
        }
    }

    /// Where the repository keeps its objects, which the scratch reads through.
    fn alternate(&self) -> PathBuf {
        let common = self.plain(&["rev-parse", "--path-format=absolute", "--git-common-dir"]);
        PathBuf::from(common.trim()).join("objects")
    }

    fn plain(&self, args: &[&str]) -> String {
        match self.run(args, None, &[]) {
            Ok(out) => out,
            Err(message) => panic!("git {args:?}: {message}"),
        }
    }

    fn run(
        &self,
        args: &[&str],
        stdin: Option<&[u8]>,
        extra: &[(&str, &str)],
    ) -> Result<String, String> {
        let mut command = Command::new("git");
        command
            .current_dir(&self.repo)
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .env("GIT_OPTIONAL_LOCKS", "0")
            .stdin(if stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (name, value) in extra {
            command.env(name, value);
        }
        let mut child = command
            .spawn()
            .unwrap_or_else(|e| panic!("could not start git {args:?}: {e}"));
        if let Some(bytes) = stdin {
            use std::io::Write;
            let Some(mut pipe) = child.stdin.take() else {
                panic!("git {args:?} was given no standard input to write to");
            };
            let _ = pipe.write_all(bytes);
        }
        let output = child
            .wait_with_output()
            .unwrap_or_else(|e| panic!("waiting for git {args:?}: {e}"));
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        } else {
            Err(format!(
                "{} {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            ))
        }
    }

    /// Every command that may write runs with the scratch index and object directory.
    fn isolated(&self, args: &[&str], stdin: Option<&[u8]>) -> Result<String, String> {
        let index = self.dir.join("index");
        let objects = self.dir.join("objects");
        let alternate = self.alternate();
        self.run(
            args,
            stdin,
            &[
                ("GIT_INDEX_FILE", &index.to_string_lossy()),
                ("GIT_OBJECT_DIRECTORY", &objects.to_string_lossy()),
                (
                    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
                    &alternate.to_string_lossy(),
                ),
            ],
        )
    }

    /// Fills the scratch index with one tree, which is the preimage a patch applies to.
    pub fn read_tree(&self, tree: &str) {
        let _ = std::fs::remove_file(self.dir.join("index"));
        self.isolated(&["read-tree", tree], None)
            .unwrap_or_else(|e| panic!("read-tree {tree}: {e}"));
    }

    /// `git apply --check --cached` and then `git apply --cached`, which is the pair
    /// `git add -p` itself runs. `Err` is git's own complaint.
    pub fn apply(&self, patch: &[u8], reverse: bool) -> Result<(), String> {
        let mut args = vec!["apply", "--cached", "--whitespace=nowarn"];
        if reverse {
            args.push("--reverse");
        }
        let mut check = args.clone();
        check.push("--check");
        self.isolated(&check, Some(patch))
            .map_err(|e| format!("apply --check refused it: {e}"))?;
        self.isolated(&args, Some(patch))
            .map(|_| ())
            .map_err(|e| format!("apply failed after --check passed: {e}"))
    }

    pub fn write_tree(&self) -> String {
        self.isolated(&["write-tree"], None)
            .unwrap_or_else(|e| panic!("write-tree: {e}"))
            .trim()
            .to_owned()
    }

    /// The mode and object id staged at `path`, or `None` when nothing is.
    pub fn staged(&self, path: &str) -> Option<(String, String)> {
        let listed = self
            .isolated(&["ls-files", "--stage", "--", path], None)
            .unwrap_or_else(|e| panic!("ls-files {path}: {e}"));
        let line = listed.lines().next()?;
        let mut fields = line.split_whitespace();
        let mode = fields.next()?.to_owned();
        let id = fields.next()?.to_owned();
        Some((mode, id))
    }

    /// Stages one object at one path without going through a patch, for the changed files
    /// a diff model deliberately has no patch for.
    pub fn stage(&self, mode: &str, id: &str, path: &str) {
        self.isolated(
            &[
                "update-index",
                "--add",
                "--cacheinfo",
                &format!("{mode},{id},{path}"),
            ],
            None,
        )
        .unwrap_or_else(|e| panic!("staging {path}: {e}"));
    }

    pub fn unstage(&self, path: &str) {
        self.isolated(&["update-index", "--force-remove", "--", path], None)
            .unwrap_or_else(|e| panic!("unstaging {path}: {e}"));
    }

    pub fn blob(&self, id: &str) -> Vec<u8> {
        let mut command = Command::new("git");
        let objects = self.dir.join("objects");
        let alternate = self.alternate();
        let output = command
            .current_dir(&self.repo)
            .args(["cat-file", "blob", id])
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .env("GIT_OBJECT_DIRECTORY", &objects)
            .env("GIT_ALTERNATE_OBJECT_DIRECTORIES", &alternate)
            .output()
            .unwrap_or_else(|e| panic!("cat-file {id}: {e}"));
        assert!(
            output.status.success(),
            "cat-file {id}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        output.stdout
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
