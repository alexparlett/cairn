//! The seal on a write.
//!
//! A `git` invocation is built as a read or a write (`crate::process`), and a
//! write can only be built from a [`WriteAuthority`]. The type has a private
//! field and one constructor, visible to `ops/` and its children alone, so the
//! compiler refuses a write anywhere else in the crate — in `reads/`, in
//! `history/`, in `process/` itself — and from outside the crate the type
//! cannot even be named (the `compile_fail` pins in the [`super`] module
//! docs). It is a token rather than a visibility on the builder because
//! `process/` cannot write `pub(in crate::ops)` on an item of its own: a
//! restricted visibility must name an ancestor of the item.
//!
//! The guard `the_runner_is_named_only_by_ops_and_reads` is the twin against
//! erosion: it refuses the constructor, a literal or an `impl` block for the
//! type anywhere outside `ops/`, and it pins the constructor's visibility and
//! the field's privacy here, so widening either fails the gate rather than
//! compiling quietly.

/// Permission to build a write invocation, which only `ops/` can construct.
///
/// Every operation takes a fresh one for each process it starts and hands it
/// to [`crate::ops::GitBinary`]'s write builder, which keeps it for the life of
/// the invocation. Not `Clone`, not `Copy` and not `Default`: there is one way
/// to get one, and it is here.
#[derive(Debug)]
pub(crate) struct WriteAuthority {
    _sealed: (),
}

impl WriteAuthority {
    /// For an operation in `ops/` about to start a write; nothing else can call it.
    pub(in crate::ops) fn new() -> Self {
        Self { _sealed: () }
    }
}

/// Writes against a stub `git` that prints what it was given, and against real
/// `git`. Here rather than in `process/` because a write needs the authority,
/// and nothing outside `ops/` can construct one — tests included.
#[cfg(all(test, unix))]
mod tests {
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{Duration, Instant, SystemTime};

    use cairn_model::AskpassToken;

    use super::WriteAuthority;
    use crate::ops::{Askpass, GitBinary, GitEnvironment};
    use crate::process::stub_git::{StubGit, discover_retrying, printed_environment};
    use crate::{Error, Repository};

    /// Answers `--version`, then prints its environment for anything else.
    fn printing_stub() -> StubGit {
        StubGit::with_git(
            "if [ \"$1\" = --version ]; then echo 'git version 2.30.0'; exit 0; fi\n\
             exec /usr/bin/env",
        )
    }

    /// The write environment of a stub on `path` with no inherited variable but
    /// `PATH`, spelled out variable by variable, plus `extra`.
    fn write_environment(path: &str, extra: &[(&str, &str)]) -> BTreeMap<String, String> {
        [
            ("GIT_ASKPASS", StubGit::HELPER),
            ("GIT_EDITOR", "false"),
            ("GIT_SEQUENCE_EDITOR", "false"),
            ("GIT_TERMINAL_PROMPT", "0"),
            ("PATH", path),
            ("SSH_ASKPASS", StubGit::HELPER),
            ("SSH_ASKPASS_REQUIRE", "force"),
        ]
        .iter()
        .chain(extra)
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect()
    }

    fn path_of(environment: &GitEnvironment) -> String {
        environment
            .get("PATH")
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_else(|| panic!("the stub environment has no PATH"))
    }

    /// PRD G3 end to end, for a write: the base with the editor pinned, no
    /// `GIT_OPTIONAL_LOCKS`, no token when none was given — exactly that, and
    /// nothing from this process.
    #[test]
    fn a_write_sees_exactly_the_write_environment_and_nothing_inherited() {
        let stub = printing_stub();
        let environment = stub.environment();
        let path = path_of(&environment);
        let git = discover_retrying(environment).unwrap();
        let output = git
            .write_invocation(WriteAuthority::new())
            .arg("print-environment")
            .run()
            .unwrap();
        assert_eq!(
            printed_environment(&output.stdout_text()),
            write_environment(&path, &[])
        );
    }

    /// The token reaches the child only on a write that was given one, and it
    /// is the only thing the authorisation adds.
    #[test]
    fn an_authorised_write_carries_its_token_and_only_that_one() {
        let stub = printing_stub();
        let environment = stub.environment();
        let path = path_of(&environment);
        let git = discover_retrying(environment).unwrap();
        let token = AskpassToken::new(format!("token-{}", std::process::id()));
        let output = git
            .write_invocation(WriteAuthority::new())
            .arg("print-environment")
            .authorized_by(&token)
            .run()
            .unwrap();
        assert_eq!(
            printed_environment(&output.stdout_text()),
            write_environment(&path, &[("CAIRN_ASKPASS_TOKEN", token.as_str())])
        );
    }

    /// A directory holding a home with no configuration in it and a repository
    /// made by real `git`, through Cairn's own write invocations; removed when
    /// the test ends.
    struct Scratch {
        root: PathBuf,
        git: GitBinary,
    }

    impl Scratch {
        fn new(name: &str) -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let root = std::env::temp_dir().join(format!(
                "cairn-ops-{name}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(root.join("home")).unwrap_or_else(|error| panic!("{error}"));
            let home = root.join("home").into_os_string();
            // The machine's `git` on this process's PATH, with a HOME that holds no
            // configuration, so a user's `commit.gpgsign` or hooks cannot decide
            // anything here.
            let environment = GitEnvironment::new(
                |name| match name {
                    "PATH" => std::env::var_os("PATH"),
                    "HOME" => Some(home.clone()),
                    _ => None,
                },
                &Askpass::new("/nonexistent/cairn-askpass", None),
            );
            let git =
                GitBinary::discover_with(environment).unwrap_or_else(|error| panic!("{error}"));
            let scratch = Self { root, git };
            scratch.write(&["init", "-q", "repo"], &scratch.root);
            for (key, value) in [
                ("user.name", "A U Thor"),
                ("user.email", "author@example.com"),
            ] {
                scratch.write(&["config", key, value], &scratch.repo());
            }
            scratch
        }

        fn repo(&self) -> PathBuf {
            self.root.join("repo")
        }

        /// Runs a write `git` in `directory` (through `-C`, so no repository need
        /// exist to run it in), panicking on failure.
        fn write(&self, args: &[&str], directory: &Path) {
            self.git
                .write_invocation(WriteAuthority::new())
                .arg("-C")
                .arg(directory)
                .args(args)
                .run()
                .unwrap_or_else(|error| panic!("git {args:?}: {error}"));
        }

        fn head(&self) -> String {
            self.git
                .read_invocation()
                .arg("-C")
                .arg(self.repo())
                .args(["rev-parse", "HEAD"])
                .run()
                .unwrap_or_else(|error| panic!("git rev-parse HEAD: {error}"))
                .stdout_text()
                .trim()
                .to_owned()
        }

        /// A script that records it ran, then outstays any test's patience: the
        /// editor a user configured, which Cairn must never wait on.
        fn hanging_editor(&self) -> (String, PathBuf) {
            let marker = self.root.join("editor-ran");
            let script = self.root.join("editor.sh");
            std::fs::write(&script, format!("touch '{}'\nsleep 5\n", marker.display()))
                .unwrap_or_else(|error| panic!("{error}"));
            // Run by `sh` rather than executed, so a parallel test's fork holding this
            // file's write descriptor cannot make it "text file busy".
            (format!("sh '{}'", script.display()), marker)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    /// How long a verb that would open an editor may take to fail: far less than
    /// the hanging editor's sleep, and far more than `git` needs on a loaded
    /// machine.
    const PROMPTLY: Duration = Duration::from_secs(4);

    /// PRD G5: `git commit` with no message fails promptly instead of waiting on
    /// an editor — the user's configured `core.editor` is never run, because
    /// `GIT_EDITOR=false` outranks it — and nothing is committed. Decisive
    /// because the configured editor records that it ran and then hangs past
    /// the deadline, so an environment without the pin fails both assertions.
    #[test]
    fn a_commit_without_a_message_fails_promptly_instead_of_opening_an_editor() {
        let scratch = Scratch::new("commit-editor");
        let repo = scratch.repo();
        std::fs::write(repo.join("tracked"), "one\n").unwrap();
        scratch.write(&["add", "tracked"], &repo);
        scratch.write(&["commit", "-q", "-m", "initial"], &repo);
        let before = scratch.head();
        let (editor, marker) = scratch.hanging_editor();
        scratch.write(&["config", "core.editor", &editor], &repo);
        std::fs::write(repo.join("tracked"), "two\n").unwrap();
        scratch.write(&["add", "tracked"], &repo);

        let handle = Repository::discover(&repo).unwrap();
        let started = Instant::now();
        let outcome = scratch
            .git
            .write_invocation(WriteAuthority::new())
            .in_repository(&handle)
            .arg("commit")
            .run();
        let took = started.elapsed();

        assert!(
            matches!(&outcome, Err(Error::GitFailed { arguments, .. }) if arguments == "commit"),
            "a commit with no message and no editor must fail: {outcome:?}"
        );
        assert!(took < PROMPTLY, "the commit waited {took:?} for an editor");
        assert!(
            !marker.exists(),
            "the user's core.editor ran: GIT_EDITOR did not outrank it"
        );
        assert_eq!(scratch.head(), before, "something was committed");
    }

    /// The same for the rebase todo list, whose editor is `GIT_SEQUENCE_EDITOR`,
    /// then `sequence.editor`, and only then `GIT_EDITOR` — so a user's
    /// `sequence.editor` would beat the commit editor's pin, and this is what
    /// the second pin is for.
    #[test]
    fn an_interactive_rebase_fails_promptly_instead_of_opening_the_sequence_editor() {
        let scratch = Scratch::new("sequence-editor");
        let repo = scratch.repo();
        for content in ["one\n", "two\n"] {
            std::fs::write(repo.join("tracked"), content).unwrap();
            scratch.write(&["add", "tracked"], &repo);
            scratch.write(&["commit", "-q", "-m", content.trim()], &repo);
        }
        let before = scratch.head();
        let (editor, marker) = scratch.hanging_editor();
        scratch.write(&["config", "sequence.editor", &editor], &repo);

        let handle = Repository::discover(&repo).unwrap();
        let started = Instant::now();
        let outcome = scratch
            .git
            .write_invocation(WriteAuthority::new())
            .in_repository(&handle)
            .args(["rebase", "-i", "HEAD~1"])
            .run();
        let took = started.elapsed();

        assert!(
            matches!(&outcome, Err(Error::GitFailed { .. })),
            "an interactive rebase with no sequence editor must fail: {outcome:?}"
        );
        assert!(took < PROMPTLY, "the rebase waited {took:?} for an editor");
        assert!(
            !marker.exists(),
            "the user's sequence.editor ran: GIT_SEQUENCE_EDITOR did not outrank it"
        );
        assert_eq!(scratch.head(), before, "the rebase moved HEAD");
    }

    /// PRD G4: a read of `git status` on a dirty repository whose index is stale
    /// leaves the index byte for byte as it was, because a read runs with
    /// `GIT_OPTIONAL_LOCKS=0`. Decisive because the same `status` as a write —
    /// the one thing that differs being the read profile — DOES rewrite the
    /// index of the same repository: without that, an index git had no reason
    /// to refresh would pass this whatever the environment said.
    #[test]
    fn a_status_read_leaves_a_stale_index_byte_identical() {
        let scratch = Scratch::new("status-read");
        let repo = scratch.repo();
        std::fs::write(repo.join("tracked"), "one\n").unwrap();
        std::fs::write(repo.join("edited"), "a\n").unwrap();
        scratch.write(&["add", "tracked", "edited"], &repo);
        scratch.write(&["commit", "-q", "-m", "initial"], &repo);
        // Stale: `tracked`'s content is what the index records but its stat
        // information is not, so `status` re-hashes it, finds it clean, and would
        // write the refreshed stat back. Dirty: an edit and an untracked file.
        std::fs::File::options()
            .write(true)
            .open(repo.join("tracked"))
            .unwrap()
            .set_modified(SystemTime::now() - Duration::from_secs(3600))
            .unwrap();
        std::fs::write(repo.join("edited"), "b\n").unwrap();
        std::fs::write(repo.join("untracked"), "c\n").unwrap();
        let index = repo.join(".git").join("index");
        let before = std::fs::read(&index).unwrap();

        let handle = Repository::discover(&repo).unwrap();
        let read = scratch
            .git
            .read_invocation()
            .in_repository(&handle)
            .args(["status", "--porcelain=v2", "-z"])
            .run()
            .unwrap();
        let records: Vec<&[u8]> = read.records().collect();
        assert!(
            records.iter().any(|record| record.ends_with(b" edited"))
                && records.contains(&b"? untracked".as_slice()),
            "status did not see the dirty working tree: {records:?}"
        );
        assert!(
            std::fs::read(&index).unwrap() == before,
            "a status read rewrote the index"
        );
        assert!(
            !repo.join(".git").join("index.lock").exists(),
            "a status read left index.lock behind"
        );

        scratch
            .git
            .write_invocation(WriteAuthority::new())
            .in_repository(&handle)
            .args(["status", "--porcelain=v2", "-z"])
            .run()
            .unwrap_or_else(|error| panic!("the same status as a write: {error}"));
        assert!(
            std::fs::read(&index).unwrap() != before,
            "the same status as a write left the index alone too, so the fixture was not \
             stale and the read above decided nothing"
        );
    }
}
