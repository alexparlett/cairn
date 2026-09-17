//! The helper binary against a real channel, and every way it must fail closed.
//!
//! Each test opens its own channel under a fresh runtime directory in the
//! temp dir — never the real `$XDG_RUNTIME_DIR` — runs the built helper with
//! an environment it constructs from nothing, and answers or refuses from a
//! thread. Secrets are generated per test, never written down.
#![cfg(unix)]

use std::ffi::OsStr;
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread::JoinHandle;

use cairn_askpass::{Channel, Error, Prompt};
use cairn_model::{AskpassToken, SOCKET_VARIABLE, Secret, TOKEN_VARIABLE};

const HELPER: &str = env!("CARGO_BIN_EXE_cairn-askpass");

/// A stand-in `$XDG_RUNTIME_DIR`, `0700`, removed when the test ends.
struct RuntimeDir {
    path: PathBuf,
}

impl RuntimeDir {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let unique = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "cairn-askpass-test-{}-{unique}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&path)
            .unwrap_or_else(|e| panic!("could not make {}: {e}", path.display()));
        Self { path }
    }
}

impl Drop for RuntimeDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// A value no test file contains: different per process and per call.
fn generated_secret() -> String {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    format!(
        "generated-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}

/// Runs the helper with exactly these variables and this prompt; nothing inherited.
fn run_helper(prompt: Option<&str>, variables: &[(&str, &OsStr)]) -> Output {
    let mut command = Command::new(HELPER);
    command
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (name, value) in variables {
        command.env(name, value);
    }
    if let Some(prompt) = prompt {
        command.arg(prompt);
    }
    command
        .output()
        .unwrap_or_else(|e| panic!("could not run {HELPER}: {e}"))
}

fn run_against(channel: &Channel, token: &AskpassToken, prompt: &str) -> Output {
    run_helper(
        Some(prompt),
        &[
            (SOCKET_VARIABLE, channel.socket_path().as_os_str()),
            (TOKEN_VARIABLE, OsStr::new(token.as_str())),
        ],
    )
}

fn failed_closed(output: &Output) {
    assert!(
        !output.status.success(),
        "the helper exited successfully: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        b"",
        "the helper wrote to stdout on a failure path: {:?}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        output.stderr.starts_with(b"cairn-askpass: "),
        "no reason on stderr: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Accepts one connection on another thread and does `serve` with it.
fn accept_once<T: Send + 'static>(
    channel: std::sync::Arc<Channel>,
    serve: impl FnOnce(Result<Prompt, Error>) -> T + Send + 'static,
) -> JoinHandle<T> {
    std::thread::spawn(move || serve(channel.accept()))
}

fn shared(runtime: &RuntimeDir) -> std::sync::Arc<Channel> {
    std::sync::Arc::new(
        Channel::open(&runtime.path).unwrap_or_else(|e| panic!("could not open a channel: {e}")),
    )
}

#[test]
fn the_helper_prints_the_answer_with_a_trailing_newline_and_nothing_else() {
    let runtime = RuntimeDir::new();
    let channel = shared(&runtime);
    let operation = channel.begin().unwrap();
    let secret = generated_secret();
    let answer = secret.clone();
    let served = accept_once(channel.clone(), move |prompt| {
        let prompt = prompt.expect("a well-formed prompt");
        let text = prompt.text().to_owned();
        prompt.answer(&Secret::from_string(answer)).unwrap();
        text
    });

    let output = run_against(
        &channel,
        operation.token(),
        "Password for 'https://x@host': ",
    );
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, format!("{secret}\n").into_bytes());
    assert_eq!(output.stderr, b"");
    assert_eq!(served.join().unwrap(), "Password for 'https://x@host': ");
}

/// git asks for a username and then a password with the same environment; one
/// token serves both, which is why it is scoped to the operation.
#[test]
fn one_operation_answers_more_than_one_prompt() {
    let runtime = RuntimeDir::new();
    let channel = shared(&runtime);
    let operation = channel.begin().unwrap();
    for prompt in [
        "Username for 'https://host': ",
        "Password for 'https://u@host': ",
    ] {
        let secret = generated_secret();
        let answer = secret.clone();
        let served = accept_once(channel.clone(), move |prompt| {
            prompt
                .unwrap()
                .answer(&Secret::from_string(answer))
                .unwrap();
        });
        let output = run_against(&channel, operation.token(), prompt);
        assert!(output.status.success());
        assert_eq!(output.stdout, format!("{secret}\n").into_bytes());
        served.join().unwrap();
    }
}

#[test]
fn without_a_prompt_the_helper_fails_closed() {
    let runtime = RuntimeDir::new();
    let channel = shared(&runtime);
    let operation = channel.begin().unwrap();
    let output = run_helper(
        None,
        &[
            (SOCKET_VARIABLE, channel.socket_path().as_os_str()),
            (TOKEN_VARIABLE, OsStr::new(operation.token().as_str())),
        ],
    );
    failed_closed(&output);
}

#[test]
fn without_the_channel_in_its_environment_the_helper_fails_closed() {
    let runtime = RuntimeDir::new();
    let channel = shared(&runtime);
    let operation = channel.begin().unwrap();
    // No variables at all: a git Cairn did not start.
    failed_closed(&run_helper(Some("Password: "), &[]));
    // The socket without a token.
    failed_closed(&run_helper(
        Some("Password: "),
        &[(SOCKET_VARIABLE, channel.socket_path().as_os_str())],
    ));
    // A token without the socket.
    failed_closed(&run_helper(
        Some("Password: "),
        &[(TOKEN_VARIABLE, OsStr::new(operation.token().as_str()))],
    ));
}

/// "App has no UI": nothing is listening at the path, or the path is stale.
#[test]
fn with_no_cairn_listening_the_helper_fails_closed() {
    let runtime = RuntimeDir::new();
    let token = AskpassToken::new(generated_secret());
    let missing = runtime.path.join("nothing-here").join("askpass");
    failed_closed(&run_helper(
        Some("Password: "),
        &[
            (SOCKET_VARIABLE, missing.as_os_str()),
            (TOKEN_VARIABLE, OsStr::new(token.as_str())),
        ],
    ));

    // A socket file left behind by a Cairn that is gone.
    let stale = {
        let channel = Channel::open(&runtime.path).unwrap();
        let path = channel.socket_path().to_owned();
        // Keep the socket file past the channel's cleanup by copying its directory name.
        let kept = runtime.path.join("stale");
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&kept)
            .unwrap();
        let kept = kept.join("askpass");
        std::os::unix::net::UnixListener::bind(&kept).unwrap();
        std::fs::set_permissions(&kept, std::fs::Permissions::from_mode(0o600)).unwrap();
        drop(channel);
        assert!(!path.exists(), "the channel left its socket behind");
        kept
    };
    // The listener above was dropped: the file exists, nobody accepts.
    let output = run_helper(
        Some("Password: "),
        &[
            (SOCKET_VARIABLE, stale.as_os_str()),
            (TOKEN_VARIABLE, OsStr::new(token.as_str())),
        ],
    );
    failed_closed(&output);
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("no Cairn listening"),
        "{:?}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// The user cancels: nothing reaches git, and the token dies with the refusal
/// so git's retry is refused too rather than asking again (PRD R2.5).
#[test]
fn a_refused_prompt_fails_closed_and_retires_the_token() {
    let runtime = RuntimeDir::new();
    let channel = shared(&runtime);
    let operation = channel.begin().unwrap();
    let served = accept_once(channel.clone(), |prompt| prompt.unwrap().refuse());
    let output = run_against(&channel, operation.token(), "Password: ");
    failed_closed(&output);
    assert!(String::from_utf8_lossy(&output.stderr).contains("declined"));
    served.join().unwrap();

    let served = accept_once(channel.clone(), |prompt| {
        assert!(matches!(prompt, Err(Error::UnknownToken)), "{prompt:?}");
    });
    failed_closed(&run_against(&channel, operation.token(), "Password: "));
    served.join().unwrap();
}

/// A prompt the application dropped without answering counts as refused.
#[test]
fn a_prompt_dropped_unanswered_is_a_refusal() {
    let runtime = RuntimeDir::new();
    let channel = shared(&runtime);
    let operation = channel.begin().unwrap();
    let served = accept_once(channel.clone(), |prompt| drop(prompt.unwrap()));
    failed_closed(&run_against(&channel, operation.token(), "Password: "));
    served.join().unwrap();
    let served = accept_once(channel.clone(), |prompt| {
        assert!(matches!(prompt, Err(Error::UnknownToken)), "{prompt:?}");
    });
    failed_closed(&run_against(&channel, operation.token(), "Password: "));
    served.join().unwrap();
}

/// "Token already used": the operation ended, or the token was never issued.
#[test]
fn a_token_for_no_live_operation_is_refused() {
    let runtime = RuntimeDir::new();
    let channel = shared(&runtime);
    let ended = channel.begin().unwrap().token().clone();
    // The `Operation` above is already dropped, and with it the token.
    for token in [ended, AskpassToken::new(generated_secret())] {
        let served = accept_once(channel.clone(), |prompt| {
            assert!(matches!(prompt, Err(Error::UnknownToken)), "{prompt:?}");
        });
        failed_closed(&run_against(&channel, &token, "Password: "));
        served.join().unwrap();
    }
}

/// A socket or directory readable beyond its owner is one the helper will not
/// hand a secret to; it never even connects.
#[test]
fn a_socket_with_the_wrong_permissions_is_refused_before_connecting() {
    let runtime = RuntimeDir::new();
    let channel = shared(&runtime);
    let operation = channel.begin().unwrap();
    let socket = channel.socket_path().to_owned();
    let directory = socket.parent().unwrap().to_owned();

    for (path, mode) in [(&socket, 0o666), (&socket, 0o640), (&directory, 0o755)] {
        let original = std::fs::metadata(path).unwrap().permissions();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
        let output = run_against(&channel, operation.token(), "Password: ");
        std::fs::set_permissions(path, original).unwrap();
        failed_closed(&output);
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("beyond its owner"),
            "{path:?} mode {mode:o}: {:?}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    // Nothing above reached the channel: had the helper connected, its request
    // would be queued ahead of this probe and `accept` would return a Prompt
    // for the still-live operation instead of the probe's refusal.
    let served = accept_once(channel.clone(), |first| {
        assert!(
            matches!(first, Err(Error::Malformed)),
            "the helper connected to a socket it should have refused: {first:?}"
        );
    });
    let mut probe = UnixStream::connect(&socket).unwrap();
    probe.write_all(b"probe").unwrap();
    probe.shutdown(std::net::Shutdown::Write).unwrap();
    let mut reply = Vec::new();
    probe.read_to_end(&mut reply).unwrap();
    assert_eq!(reply, b"refused\n");
    served.join().unwrap();

    // Not a socket at all.
    let file = runtime.path.join("plain");
    std::fs::write(&file, b"").unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
    let output = run_helper(
        Some("Password: "),
        &[
            (SOCKET_VARIABLE, file.as_os_str()),
            (TOKEN_VARIABLE, OsStr::new(operation.token().as_str())),
        ],
    );
    failed_closed(&output);
    assert!(String::from_utf8_lossy(&output.stderr).contains("is not the Cairn askpass socket"));
}

#[test]
fn what_the_channel_creates_is_owner_only_and_gone_on_drop() {
    let runtime = RuntimeDir::new();
    let channel = Channel::open(&runtime.path).unwrap();
    let socket = channel.socket_path().to_owned();
    let directory = socket.parent().unwrap().to_owned();
    assert_eq!(directory.parent(), Some(runtime.path.as_path()));
    assert!(
        directory
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with(&format!("cairn-{}-", std::process::id()))
    );
    let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&directory), 0o700);
    assert_eq!(mode(&socket), 0o600);
    assert!(
        std::fs::metadata(&socket).unwrap().file_type().is_socket(),
        "not a socket"
    );

    // Two channels in one process do not collide.
    let second = Channel::open(&runtime.path).unwrap();
    assert_ne!(second.socket_path(), socket);

    drop(channel);
    assert!(!socket.exists(), "the socket was left behind");
    assert!(!directory.exists(), "the directory was left behind");
    assert!(second.socket_path().exists());
}

#[test]
fn without_a_runtime_directory_there_is_no_channel() {
    let runtime = RuntimeDir::new();
    let missing = runtime.path.join("absent");
    assert!(matches!(
        Channel::open(&missing),
        Err(Error::NoRuntimeDirectory { path, .. }) if path == missing
    ));
    let file = runtime.path.join("file");
    std::fs::write(&file, b"").unwrap();
    assert!(matches!(
        Channel::open(&file),
        Err(Error::NoRuntimeDirectory { path, .. }) if path == file
    ));
    // Nothing was left behind by either refusal.
    let entries: Vec<_> = std::fs::read_dir(&runtime.path)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(entries, [OsStr::new("file")]);
}

/// Something that is not a helper connects: refused, reported, and the
/// channel goes on serving.
#[test]
fn a_connection_that_is_not_a_helper_is_refused_and_serving_continues() {
    let runtime = RuntimeDir::new();
    let channel = shared(&runtime);
    let served = accept_once(channel.clone(), |prompt| {
        assert!(matches!(prompt, Err(Error::Malformed)), "{prompt:?}");
    });
    let mut stranger = UnixStream::connect(channel.socket_path()).unwrap();
    stranger.write_all(b"GET / HTTP/1.0\r\n\r\n").unwrap();
    stranger.shutdown(std::net::Shutdown::Write).unwrap();
    let mut reply = Vec::new();
    stranger.read_to_end(&mut reply).unwrap();
    assert_eq!(reply, b"refused\n");
    served.join().unwrap();

    let operation = channel.begin().unwrap();
    let secret = generated_secret();
    let answer = secret.clone();
    let served = accept_once(channel.clone(), move |prompt| {
        prompt
            .unwrap()
            .answer(&Secret::from_string(answer))
            .unwrap();
    });
    let output = run_against(&channel, operation.token(), "Password: ");
    assert!(output.status.success());
    assert_eq!(output.stdout, format!("{secret}\n").into_bytes());
    served.join().unwrap();
}
