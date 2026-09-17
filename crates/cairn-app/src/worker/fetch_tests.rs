//! The fetch and prompt wiring, end to end through the real boundary: real
//! `git`, the built helper, the channel the worker opened, a remote that
//! demands a credential, and the window's answer travelling back as a value.
//!
//! No `Command` here — the mutation guard scans this crate's tests — so the
//! remote is a loopback HTTP listener in this process that answers every
//! request with `401`: enough to make git ask, and to see whether what it
//! then sent carried the answer. Repositories are built with `std::fs`.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex, PoisonError};
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};

use cairn_model::{PromptKind, Secret};

use super::askpass::{PromptId, Reply};
use super::pool::{Replier, RepositoryHandle, Updates, open_with};
use super::request::{Request, Update};
use super::startup::Startup;

/// How long one wait on the boundary may take before the test is called hung
/// (issue #21): generous, since the fetch tests run real `git` against loopback
/// remotes, but finite, so a hang is a red test with a name rather than a
/// stalled suite.
pub(super) const WAIT: Duration = Duration::from_secs(60);

/// Drives a future on this thread. On `std::task::Wake`, since `unsafe` is forbidden.
pub(super) fn block_on<F: Future>(future: F) -> F::Output {
    struct Unpark(std::thread::Thread);
    impl std::task::Wake for Unpark {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Waker::from(Arc::new(Unpark(std::thread::current())));
    woken_by(&waker, future)
}

/// [`block_on`] with the waker supplied. Panics after [`WAIT`] with nothing ready.
pub(super) fn woken_by<F: Future>(waker: &Waker, future: F) -> F::Output {
    let mut cx = Context::from_waker(waker);
    let mut future = std::pin::pin!(future);
    let deadline = Instant::now() + WAIT;
    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(value) => return value,
            Poll::Pending => {
                let now = Instant::now();
                assert!(
                    now < deadline,
                    "nothing arrived on the boundary in {WAIT:?}: whatever should have sent or \
                     woken is parked, or the stream was left open"
                );
                std::thread::park_timeout(deadline - now);
            }
        }
    }
}

/// A stand-in `$XDG_RUNTIME_DIR`, `0700`, removed when the test ends.
pub(super) struct RuntimeDir {
    pub(super) path: PathBuf,
}

impl RuntimeDir {
    pub(super) fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "cairn-app-runtime-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&path);
        if let Err(error) = std::fs::DirBuilder::new().mode(0o700).create(&path) {
            panic!("could not make {}: {error}", path.display());
        }
        Self { path }
    }
}

impl Drop for RuntimeDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// `target/<profile>/cairn-askpass`, two directories up from this test
/// executable. Built by `cargo test --workspace` (and by the gate) before any
/// test runs; a `-p cairn-app` run needs `cargo build -p cairn-askpass` first,
/// which the failure names — this crate's tests may not run cargo themselves.
pub(super) fn built_helper() -> PathBuf {
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(error) => panic!("no current exe: {error}"),
    };
    let Some(profile) = exe.parent().and_then(Path::parent) else {
        panic!("{} is not under target/<profile>/deps", exe.display());
    };
    let helper = profile.join(cairn_model::HELPER_PROGRAM);
    assert!(
        helper.is_file(),
        "the askpass helper is not built at {}; run `cargo build -p cairn-askpass`",
        helper.display()
    );
    helper
}

/// Built with `std::fs`, not `git init`: the mutation guard scans this crate's tests too.
pub(super) struct UnbornRepository {
    pub(super) path: PathBuf,
}

impl UnbornRepository {
    pub(super) fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(name);
        let _ = std::fs::remove_dir_all(&path);
        let dot = path.join(".git");
        for inside in ["objects/info", "objects/pack", "refs/heads", "refs/tags"] {
            if let Err(error) = std::fs::create_dir_all(dot.join(inside)) {
                panic!("building {}: {error}", dot.join(inside).display());
            }
        }
        if let Err(error) = std::fs::write(dot.join("HEAD"), "ref: refs/heads/main\n") {
            panic!("writing HEAD: {error}");
        }
        if let Err(error) = std::fs::write(
            dot.join("config"),
            "[core]\n\trepositoryformatversion = 0\n\tbare = false\n",
        ) {
            panic!("writing config: {error}");
        }
        Self { path }
    }
}

impl Drop for UnbornRepository {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// A bare repository, built with `std::fs`, whose `origin` is `remote` and
/// whose `main` is the remote's `HEAD` after a fetch — whatever that HEAD is.
/// The remote is the Cairn checkout, and on CI that checkout has no `main`:
/// `actions/checkout` leaves a `push` run on its one branch and a
/// `pull_request` run detached with no local branch at all, so a
/// `refs/heads/*` refspec fetched the wrong branch or nothing (and the
/// fixture's `HEAD` stayed unborn) while the same test passed on every
/// developer machine. `HEAD` as the source is what every checkout has.
struct BareRepository {
    path: PathBuf,
}

impl BareRepository {
    fn new(name: &str, remote: &Path) -> Self {
        let path = std::env::temp_dir().join(name);
        let _ = std::fs::remove_dir_all(&path);
        for inside in ["objects/info", "objects/pack", "refs/heads", "refs/tags"] {
            if let Err(error) = std::fs::create_dir_all(path.join(inside)) {
                panic!("building {}: {error}", path.join(inside).display());
            }
        }
        if let Err(error) = std::fs::write(path.join("HEAD"), "ref: refs/heads/main\n") {
            panic!("writing HEAD: {error}");
        }
        let config = format!(
            "[core]\n\trepositoryformatversion = 0\n\tbare = true\n\
             [remote \"origin\"]\n\turl = {}\n\tfetch = +HEAD:refs/heads/main\n",
            remote.display()
        );
        if let Err(error) = std::fs::write(path.join("config"), config) {
            panic!("writing config: {error}");
        }
        Self { path }
    }
}

impl Drop for BareRepository {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// A `HOME` with a `.gitconfig` that resets the credential helper list, so
/// nothing the machine has configured answers (or opens a keyring) for a test.
struct Home {
    path: PathBuf,
}

impl Home {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "cairn-app-home-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&path);
        if let Err(error) = std::fs::create_dir_all(&path) {
            panic!("could not make {}: {error}", path.display());
        }
        if let Err(error) = std::fs::write(path.join(".gitconfig"), "[credential]\n\thelper = \n") {
            panic!("could not write .gitconfig: {error}");
        }
        Self { path }
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// A remote that wants a credential and never accepts one: `401` to
/// everything, recording the `Authorization` header of each request.
struct Demanding {
    addr: SocketAddr,
    authorizations: Arc<Mutex<Vec<Option<String>>>>,
    stopping: Arc<AtomicBool>,
    serving: Option<std::thread::JoinHandle<()>>,
}

impl Demanding {
    fn new() -> Self {
        let listener = match TcpListener::bind("127.0.0.1:0") {
            Ok(listener) => listener,
            Err(error) => panic!("bind: {error}"),
        };
        let addr = match listener.local_addr() {
            Ok(addr) => addr,
            Err(error) => panic!("local_addr: {error}"),
        };
        let authorizations = Arc::new(Mutex::new(Vec::new()));
        let stopping = Arc::new(AtomicBool::new(false));
        let serving = std::thread::spawn({
            let authorizations = Arc::clone(&authorizations);
            let stopping = Arc::clone(&stopping);
            move || {
                for connection in listener.incoming() {
                    if stopping.load(Ordering::SeqCst) {
                        break;
                    }
                    let Ok(mut stream) = connection else { continue };
                    let head = read_head(&mut stream);
                    if head.is_empty() {
                        continue;
                    }
                    let authorization = head
                        .lines()
                        .filter_map(|line| line.split_once(':'))
                        .find(|(name, _)| name.trim().eq_ignore_ascii_case("authorization"))
                        .map(|(_, value)| value.trim().to_owned());
                    authorizations
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .push(authorization);
                    let _ = stream.write_all(
                        b"HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"cairn\"\r\n\
                          Content-Length: 0\r\nConnection: close\r\n\r\n",
                    );
                }
            }
        });
        Self {
            addr,
            authorizations,
            stopping,
            serving: Some(serving),
        }
    }

    fn url(&self) -> String {
        format!("http://{}/repo.git", self.addr)
    }

    fn authorizations(&self) -> Vec<Option<String>> {
        self.authorizations
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl Drop for Demanding {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.addr);
        if let Some(serving) = self.serving.take() {
            let _ = serving.join();
        }
    }
}

/// The request line and headers; empty for a connection that sent nothing.
fn read_head(stream: &mut TcpStream) -> String {
    let mut bytes = Vec::new();
    let mut byte = [0u8; 1];
    while !bytes.ends_with(b"\r\n\r\n") && bytes.len() < 64 * 1024 {
        match stream.read(&mut byte) {
            Ok(1) => bytes.push(byte[0]),
            _ => break,
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

/// A repository whose `origin` is `url`, built with `std::fs`.
fn with_origin(name: &str, url: &str) -> UnbornRepository {
    let fixture = UnbornRepository::new(name);
    let config = fixture.path.join(".git/config");
    let written = std::fs::write(
        &config,
        format!(
            "[core]\n\trepositoryformatversion = 0\n\tbare = false\n\
             [remote \"origin\"]\n\turl = {url}\n\tfetch = +refs/heads/*:refs/remotes/origin/*\n"
        ),
    );
    if let Err(error) = written {
        panic!("writing {}: {error}", config.display());
    }
    fixture
}

/// The whole boundary over `repository`, with `home` as `HOME` and a fresh runtime directory.
fn boundary(
    repository: &Path,
    home: &Path,
    runtime: &RuntimeDir,
) -> (RepositoryHandle, Updates, Replier) {
    let home = home.to_owned();
    let runtime = runtime.path.clone();
    let startup = Startup::new(
        move |name| match name {
            "PATH" => std::env::var_os("PATH"),
            "HOME" => Some(home.clone().into_os_string()),
            "XDG_RUNTIME_DIR" => Some(runtime.clone().into_os_string()),
            _ => None,
        },
        built_helper(),
    );
    match open_with(repository, startup) {
        Ok(opened) => opened,
        Err(error) => panic!("starting the worker: {error}"),
    }
}

/// Reads updates until `stop` says so, bounded, returning everything seen.
fn collect_until(updates: &mut Updates, stop: impl Fn(&Update) -> bool) -> Vec<Update> {
    let (told, verdict) = channel::<()>();
    let deadline = std::thread::spawn(move || {
        if verdict.recv_timeout(Duration::from_secs(60)).is_err() {
            panic!("no update matched within 60 seconds");
        }
    });
    let mut seen = Vec::new();
    loop {
        let Some(update) = block_on(updates.next()) else {
            let _ = told.send(());
            let _ = deadline.join();
            panic!("the stream ended before the expected update; saw {seen:?}");
        };
        let done = stop(&update);
        seen.push(update);
        if done {
            break;
        }
    }
    let _ = told.send(());
    let _ = deadline.join();
    seen
}

fn prompt_in(seen: &[Update]) -> Option<(PromptId, String)> {
    seen.iter().find_map(|update| match update {
        Update::Prompt { id, text } => Some((*id, text.clone())),
        _ => None,
    })
}

/// Standard base64 with padding, for comparing the `Authorization` header.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for group in bytes.chunks(3) {
        let mut triple = [0u8; 3];
        triple[..group.len()].copy_from_slice(group);
        let n = u32::from_be_bytes([0, triple[0], triple[1], triple[2]]);
        for i in 0..4 {
            if i <= group.len() {
                out.push(ALPHABET[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[test]
fn base64_matches_the_standard_encoding() {
    assert_eq!(base64(b"user:pass"), "dXNlcjpwYXNz");
    assert_eq!(base64(b"fo"), "Zm8=");
}

/// URL-safe, since git puts the username into the password prompt's URL.
fn generated(what: &str) -> String {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    format!(
        "generated-{what}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}

/// PRD R4: a fetch runs off the UI thread through the boundary, reports its
/// progress, and the history on screen is re-asked for from the new refs —
/// the cache contract's first real test: the gix handle the worker holds
/// serves the new commits after a `git` subprocess wrote them.
#[test]
fn a_fetch_reports_progress_finishes_and_the_history_reloads_from_the_new_refs() {
    // The Cairn checkout is the remote; a bare fixture takes its HEAD as its `main`.
    let checkout = match cairn_git::SharedRepository::discover(env!("CARGO_MANIFEST_DIR")) {
        Ok(shared) => shared.workdir().map(Path::to_owned),
        Err(error) => panic!("opening the Cairn checkout: {error}"),
    };
    let Some(checkout) = checkout else {
        panic!("the Cairn checkout has no working tree");
    };
    // Bare, so its branch may be written by a fetch; git refuses to fetch into a
    // checked-out branch, unborn or not.
    let fixture = BareRepository::new("cairn-fetch-target", &checkout);
    let (home, runtime) = (Home::new(), RuntimeDir::new());
    let (handle, mut updates, _answer) = boundary(&fixture.path, &home.path, &runtime);

    handle.submit(Request::ListRemotes);
    match block_on(updates.next()) {
        Some(Update::Remotes { remotes }) => {
            assert_eq!(remotes.len(), 1);
            assert_eq!(remotes[0].name, "origin");
            assert_eq!(
                remotes[0].url.as_deref(),
                Some(checkout.display().to_string()).as_deref()
            );
        }
        other => panic!("expected the remotes, got {other:?}"),
    }
    handle.submit(Request::OpenHistory { rows: 8 });
    match block_on(updates.next()) {
        Some(Update::Rows { rows, complete }) => {
            assert!(
                rows.is_empty() && complete,
                "an unborn HEAD had rows: {rows:?}"
            );
        }
        other => panic!("expected an empty history before the fetch, got {other:?}"),
    }

    handle.submit(Request::Fetch {
        remote: "origin".to_owned(),
    });
    let seen = collect_until(&mut updates, |update| {
        matches!(
            update,
            Update::FetchFinished { .. }
                | Update::FetchFailed { .. }
                | Update::FetchCancelled { .. }
        )
    });
    assert_eq!(
        seen.first(),
        Some(&Update::FetchStarted {
            remote: "origin".to_owned()
        })
    );
    assert!(
        seen.iter()
            .any(|update| matches!(update, Update::FetchProgress { .. })),
        "no progress was reported: {seen:?}"
    );
    assert_eq!(
        seen.last(),
        Some(&Update::FetchFinished {
            remote: "origin".to_owned(),
            refreshed: true
        }),
        "{seen:?}"
    );

    // The same worker, the same gix handle: the history it serves now is the fetched one.
    handle.submit(Request::OpenHistory { rows: 3 });
    match block_on(updates.next()) {
        Some(Update::Rows { rows, .. }) => assert_eq!(
            rows.len(),
            3,
            "the history did not reload from the fetched refs"
        ),
        other => panic!("expected the fetched history, got {other:?}"),
    }
    drop(handle);
}

/// PRD B3 through the boundary: git asks, the prompt reaches the window as a
/// value, the answer goes back as a `Secret`, and what git then sends the
/// remote carries it. The remote never accepts, so the fetch ends failed and
/// the failure carries no secret.
#[test]
fn a_prompt_reaches_the_window_as_a_value_and_its_answer_reaches_git() {
    let remote = Demanding::new();
    let fixture = with_origin("cairn-prompted-fetch", &remote.url());
    let (home, runtime) = (Home::new(), RuntimeDir::new());
    let (handle, mut updates, answer) = boundary(&fixture.path, &home.path, &runtime);

    handle.submit(Request::Fetch {
        remote: "origin".to_owned(),
    });
    let seen = collect_until(&mut updates, |u| matches!(u, Update::Prompt { .. }));
    let Some((id, text)) = prompt_in(&seen) else {
        panic!("no prompt: {seen:?}");
    };
    assert_eq!(PromptKind::of(&text), PromptKind::Username, "{text:?}");
    assert!(
        text.contains(&remote.addr.to_string()),
        "the prompt does not name the URL: {text}"
    );
    let username = generated("user");
    answer(Reply::Provide {
        prompt: id,
        secret: Secret::from_string(username.clone()),
    });

    let seen = collect_until(&mut updates, |u| matches!(u, Update::Prompt { .. }));
    let Some((id, text)) = prompt_in(&seen) else {
        panic!("no second prompt: {seen:?}");
    };
    assert_eq!(PromptKind::of(&text), PromptKind::Password, "{text:?}");
    assert!(
        text.contains(&username),
        "the password prompt does not name the user: {text}"
    );
    let password = generated("password");
    answer(Reply::Provide {
        prompt: id,
        secret: Secret::from_string(password.clone()),
    });

    let seen = collect_until(&mut updates, |u| {
        matches!(u, Update::FetchFailed { .. } | Update::FetchFinished { .. })
    });
    match seen.last() {
        Some(Update::FetchFailed { message, .. }) => {
            assert!(
                !message.contains(&password),
                "the failure carries the password: {message}"
            );
            assert!(
                !message.contains("could not have asked"),
                "prompting was available, yet the failure says otherwise: {message}"
            );
        }
        other => panic!("expected the remote's refusal, got {other:?}"),
    }
    // The header must carry exactly what the window answered, not merely a credential.
    let expected = format!(
        "Basic {}",
        base64(format!("{username}:{password}").as_bytes())
    );
    assert!(
        remote
            .authorizations()
            .iter()
            .flatten()
            .any(|a| *a == expected),
        "git never sent the credential it was given: {:?}",
        remote.authorizations()
    );
    drop(handle);
}

/// PRD B5 through the boundary: the window declines; the fetch fails once,
/// nothing asks again, and the worker goes on serving.
#[test]
fn refusing_a_prompt_fails_the_fetch_once_and_the_worker_carries_on() {
    let remote = Demanding::new();
    let fixture = with_origin("cairn-refused-fetch", &remote.url());
    let (home, runtime) = (Home::new(), RuntimeDir::new());
    let (handle, mut updates, answer) = boundary(&fixture.path, &home.path, &runtime);

    handle.submit(Request::Fetch {
        remote: "origin".to_owned(),
    });
    let seen = collect_until(&mut updates, |u| matches!(u, Update::Prompt { .. }));
    let Some((id, _)) = prompt_in(&seen) else {
        panic!("no prompt: {seen:?}");
    };
    let started = Instant::now();
    answer(Reply::Refuse { prompt: id });
    let seen = collect_until(&mut updates, |u| {
        matches!(u, Update::FetchFailed { .. } | Update::FetchFinished { .. })
    });
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "a refusal took {:?}",
        started.elapsed()
    );
    assert!(
        matches!(seen.last(), Some(Update::FetchFailed { .. })),
        "{seen:?}"
    );
    assert!(
        prompt_in(&seen).is_none(),
        "git asked again after the refusal: {seen:?}"
    );
    assert!(
        remote.authorizations().iter().all(Option::is_none),
        "a refused prompt still sent something: {:?}",
        remote.authorizations()
    );

    // Still serving: a query is answered, and a second fetch asks afresh.
    handle.submit(Request::OpenHistory { rows: 8 });
    match block_on(updates.next()) {
        Some(Update::Rows { .. }) => {}
        other => panic!("the worker stopped serving after a refused fetch: {other:?}"),
    }
    handle.submit(Request::Fetch {
        remote: "origin".to_owned(),
    });
    let seen = collect_until(&mut updates, |u| matches!(u, Update::Prompt { .. }));
    let Some((id, _)) = prompt_in(&seen) else {
        panic!("no prompt on the second fetch: {seen:?}");
    };
    answer(Reply::Refuse { prompt: id });
    collect_until(&mut updates, |u| matches!(u, Update::FetchFailed { .. }));
    drop(handle);
}

/// PRD R4.3: cancelling kills git while it waits on the prompt; the outcome
/// says cancelled, the dialog's refusal afterwards is harmless, and the
/// worker goes on serving.
#[test]
fn cancelling_a_fetch_that_waits_on_a_prompt_ends_it_as_cancelled() {
    let remote = Demanding::new();
    let fixture = with_origin("cairn-cancelled-fetch", &remote.url());
    let (home, runtime) = (Home::new(), RuntimeDir::new());
    let (handle, mut updates, answer) = boundary(&fixture.path, &home.path, &runtime);

    handle.submit(Request::Fetch {
        remote: "origin".to_owned(),
    });
    let seen = collect_until(&mut updates, |u| matches!(u, Update::Prompt { .. }));
    let Some((id, _)) = prompt_in(&seen) else {
        panic!("no prompt: {seen:?}");
    };
    let started = Instant::now();
    handle.submit(Request::CancelFetch);
    let seen = collect_until(&mut updates, |u| {
        matches!(
            u,
            Update::FetchCancelled { .. }
                | Update::FetchFailed { .. }
                | Update::FetchFinished { .. }
        )
    });
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "the cancel took {:?}",
        started.elapsed()
    );
    assert_eq!(
        seen.last(),
        Some(&Update::FetchCancelled {
            remote: "origin".to_owned(),
            refreshed: false
        }),
        "{seen:?}"
    );
    // What closing the dialog does; the helper it frees belongs to a git that is gone.
    answer(Reply::Refuse { prompt: id });

    handle.submit(Request::OpenHistory { rows: 8 });
    match block_on(updates.next()) {
        Some(Update::Rows { .. }) => {}
        other => panic!("the worker stopped serving after a cancelled fetch: {other:?}"),
    }
    drop(handle);
}

/// Shutdown unblocks the acceptor: the stream ends, and the socket directory
/// the channel made is gone with it.
#[test]
fn letting_go_of_the_repository_ends_the_acceptor_and_removes_its_socket() {
    let fixture = UnbornRepository::new("cairn-shutdown-acceptor");
    let (home, runtime) = (Home::new(), RuntimeDir::new());
    let (handle, mut updates, answer) = boundary(&fixture.path, &home.path, &runtime);
    handle.submit(Request::OpenHistory { rows: 1 });
    match block_on(updates.next()) {
        Some(Update::Rows { .. }) => {}
        other => panic!("{other:?}"),
    }
    let made: Vec<_> = std::fs::read_dir(&runtime.path)
        .into_iter()
        .flatten()
        .flatten()
        .collect();
    assert_eq!(made.len(), 1, "the channel did not make its directory");

    drop(handle);
    drop(answer);
    assert!(
        block_on(updates.next()).is_none(),
        "the stream did not end: the acceptor is still blocked in accept"
    );
    // The channel is dropped by whichever thread let go of it last, a beat after the
    // stream ends.
    let started = Instant::now();
    let left = loop {
        let left: Vec<_> = std::fs::read_dir(&runtime.path)
            .into_iter()
            .flatten()
            .flatten()
            .collect();
        if left.is_empty() || started.elapsed() > Duration::from_secs(5) {
            break left;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(
        left.is_empty(),
        "the socket directory was left behind: {left:?}"
    );
}

/// A prompt left waiting at shutdown is refused, not left hanging: the git
/// behind it fails closed.
#[test]
fn a_prompt_waiting_at_shutdown_is_refused() {
    let remote = Demanding::new();
    let fixture = with_origin("cairn-shutdown-prompt", &remote.url());
    let (home, runtime) = (Home::new(), RuntimeDir::new());
    let (handle, mut updates, answer) = boundary(&fixture.path, &home.path, &runtime);
    handle.submit(Request::Fetch {
        remote: "origin".to_owned(),
    });
    collect_until(&mut updates, |u| matches!(u, Update::Prompt { .. }));

    // Only the window's answering end goes first: the refusal, not the kill, must end it.
    let started = Instant::now();
    drop(answer);
    let rest = collect_until(&mut updates, |u| {
        matches!(
            u,
            Update::FetchCancelled { .. } | Update::FetchFailed { .. }
        )
    });
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "shutdown waited {:?} on a prompt nobody will answer",
        started.elapsed()
    );
    assert!(
        matches!(rest.last(), Some(Update::FetchFailed { .. })),
        "the window letting go did not refuse the waiting prompt (git would then have \
         failed on its own): {rest:?}"
    );
    drop(handle);
    assert!(block_on(updates.next()).is_none(), "the stream did not end");
}

/// The answer channel is a value the window holds; a receiver is never handed
/// out, so nothing on a render path can wait on it.
#[test]
fn the_answerer_never_blocks_even_with_nobody_listening() {
    let (sender, receiver) = channel::<Reply>();
    drop::<Receiver<Reply>>(receiver);
    let answerer: Replier = Rc::new(move |answer| {
        let _ = sender.send(answer);
    });
    let started = Instant::now();
    answerer(Reply::Refuse {
        prompt: PromptId::for_tests(1),
    });
    assert!(started.elapsed() < Duration::from_millis(50));
}

/// A fetch that fails where nothing could have answered a prompt says why, at the
/// boundary: no runtime directory means no channel, and the failure names it.
#[test]
fn a_failure_with_no_channel_names_why_nothing_could_ask() {
    let remote = Demanding::new();
    let fixture = with_origin("cairn-no-channel-fetch", &remote.url());
    let home = Home::new();
    let startup = Startup::new(
        {
            let home = home.path.clone();
            move |name| match name {
                "PATH" => std::env::var_os("PATH"),
                "HOME" => Some(home.clone().into_os_string()),
                _ => None,
            }
        },
        built_helper(),
    );
    let (handle, mut updates, _reply) = match open_with(&fixture.path, startup) {
        Ok(opened) => opened,
        Err(error) => panic!("starting the worker: {error}"),
    };
    handle.submit(Request::Fetch {
        remote: "origin".to_owned(),
    });
    let seen = collect_until(&mut updates, |u| {
        matches!(u, Update::FetchFailed { .. } | Update::FetchFinished { .. })
    });
    match seen.last() {
        Some(Update::FetchFailed { message, .. }) => assert!(
            message.contains("XDG_RUNTIME_DIR"),
            "the failure does not say why nothing could ask: {message}"
        ),
        other => panic!("expected the fetch to fail closed, got {other:?}"),
    }
    assert!(
        prompt_in(&seen).is_none(),
        "a prompt arrived with no channel: {seen:?}"
    );
    drop(handle);
}
