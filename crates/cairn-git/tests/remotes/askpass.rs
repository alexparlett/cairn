//! A real askpass channel, answered from a test thread, and the built helper.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;

use cairn_askpass::{Channel, Error};
use cairn_model::Secret;

/// The helper binary git will run: `target/<profile>/cairn-askpass`, two
/// directories up from this test executable. `cargo test --workspace` builds
/// it before any test runs; a `-p cairn-git` run builds it here instead.
pub fn helper_binary() -> PathBuf {
    let exe = std::env::current_exe().unwrap_or_else(|e| panic!("no current exe: {e}"));
    let profile = exe
        .parent()
        .and_then(Path::parent)
        .unwrap_or_else(|| panic!("{} is not under target/<profile>/deps", exe.display()));
    let helper = profile.join(cairn_model::HELPER_PROGRAM);
    if !helper.is_file() {
        let mut build = Command::new(env!("CARGO"));
        build.args(["build", "-p", "cairn-askpass"]);
        if profile.file_name().is_some_and(|name| name == "release") {
            build.arg("--release");
        }
        let built = build.status();
        assert!(
            built.is_ok_and(|status| status.success()) && helper.is_file(),
            "the askpass helper is not built at {}; run `cargo build -p cairn-askpass`",
            helper.display()
        );
    }
    helper
}

/// What answers each prompt: the text to hand back, or `None` to refuse it.
pub type Answers = Box<dyn Fn(&str) -> Option<String> + Send + 'static>;

/// One channel under a private runtime directory, served by a thread that
/// answers with `answers` and records every prompt it was asked.
pub struct Askpass {
    channel: Arc<Channel>,
    runtime: PathBuf,
    prompts: Arc<Mutex<Vec<String>>>,
    stopping: Arc<AtomicBool>,
    serving: Option<JoinHandle<()>>,
}

impl Askpass {
    pub fn serving(answers: Answers) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let runtime = std::env::temp_dir().join(format!(
            "cairn-fetch-runtime-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&runtime);
        {
            use std::os::unix::fs::DirBuilderExt;
            std::fs::DirBuilder::new()
                .mode(0o700)
                .create(&runtime)
                .unwrap_or_else(|e| panic!("could not make {}: {e}", runtime.display()));
        }
        let channel = Arc::new(
            Channel::open(&runtime).unwrap_or_else(|e| panic!("could not open a channel: {e}")),
        );
        let prompts = Arc::new(Mutex::new(Vec::new()));
        let stopping = Arc::new(AtomicBool::new(false));
        let serving = std::thread::spawn({
            let channel = Arc::clone(&channel);
            let prompts = Arc::clone(&prompts);
            let stopping = Arc::clone(&stopping);
            move || {
                loop {
                    match channel.accept() {
                        Ok(prompt) => {
                            let text = prompt.text().to_owned();
                            prompts
                                .lock()
                                .unwrap_or_else(PoisonError::into_inner)
                                .push(text.clone());
                            match answers(&text) {
                                Some(answer) => {
                                    let _ = prompt.answer(&Secret::from_string(answer));
                                }
                                None => prompt.refuse(),
                            }
                        }
                        // A helper for a retired token was refused on the wire; keep serving.
                        Err(Error::UnknownToken) => {}
                        Err(Error::Malformed) => {
                            if stopping.load(Ordering::SeqCst) {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            }
        });
        Self {
            channel,
            runtime,
            prompts,
            stopping,
            serving: Some(serving),
        }
    }

    pub fn channel(&self) -> &Channel {
        &self.channel
    }

    pub fn socket_path(&self) -> &Path {
        self.channel.socket_path()
    }

    /// Every prompt a helper presented, in order.
    pub fn prompts(&self) -> Vec<String> {
        self.prompts
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl Drop for Askpass {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::SeqCst);
        // An empty connection is a malformed request, which wakes `accept`.
        let _ = std::os::unix::net::UnixStream::connect(self.channel.socket_path());
        if let Some(serving) = self.serving.take() {
            let _ = serving.join();
        }
        let _ = std::fs::remove_dir_all(&self.runtime);
    }
}

/// Whether any live process still carries `socket` in its environment — a
/// `git` or a helper left behind by a cancel. Reads `/proc`, so Linux only;
/// elsewhere it reports nothing left.
pub fn processes_pointed_at(socket: &Path) -> Vec<u32> {
    let needle = format!("{}={}", cairn_model::SOCKET_VARIABLE, socket.display());
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().to_str()?.parse::<u32>().ok())
        .filter(|pid| *pid != std::process::id())
        .filter(|pid| {
            std::fs::read(format!("/proc/{pid}/environ")).is_ok_and(|environ| {
                environ
                    .split(|byte| *byte == 0)
                    .any(|entry| entry == needle.as_bytes())
            })
        })
        .collect()
}

/// Polls [`processes_pointed_at`] until it is empty or `timeout` passes.
pub fn wait_for_no_process_pointed_at(socket: &Path, timeout: std::time::Duration) -> Vec<u32> {
    let started = std::time::Instant::now();
    loop {
        let left = processes_pointed_at(socket);
        if left.is_empty() || started.elapsed() > timeout {
            return left;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
}

/// Caught by: the scan matching nothing, which would make every "nothing left
/// behind" assertion pass vacuously.
#[test]
fn a_process_carrying_the_socket_in_its_environment_is_seen_until_it_is_gone() {
    let socket = Path::new("/nonexistent/cairn-positive-control/askpass");
    assert!(processes_pointed_at(socket).is_empty());
    let mut child = Command::new("sleep")
        .arg("30")
        .env(cairn_model::SOCKET_VARIABLE, socket)
        .spawn()
        .unwrap_or_else(|e| panic!("could not spawn sleep: {e}"));
    let seen = processes_pointed_at(socket);
    assert_eq!(seen, [child.id()], "the scan does not see a live process");
    let _ = child.kill();
    let _ = child.wait();
    assert!(
        wait_for_no_process_pointed_at(socket, std::time::Duration::from_secs(2)).is_empty(),
        "a reaped process is still reported"
    );
}
