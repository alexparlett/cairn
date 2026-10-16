//! An HTTP remote that demands a credential: a minimal HTTP/1.1 server in
//! this process, fronting `git http-backend` as CGI behind Basic auth. Nothing
//! but `std`; the credential is generated per fixture and never written down.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;

use crate::fixtures::Fixture;

/// One request as the server saw it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seen {
    pub path: String,
    /// The Basic-auth username presented, when the credential was right.
    pub authorized_as: Option<String>,
}

pub struct HttpRemote {
    url: String,
    username: String,
    password: String,
    root: PathBuf,
    addr: SocketAddr,
    seen: Arc<Mutex<Vec<Seen>>>,
    stopping: Arc<AtomicBool>,
    serving: Option<JoinHandle<()>>,
}

impl HttpRemote {
    /// A bare clone of `source`, served at `http://127.0.0.1:<port>/repo.git`
    /// behind a credential only this fixture knows.
    pub fn of(source: &Fixture) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let unique = NEXT.fetch_add(1, Ordering::Relaxed);
        let root =
            std::env::temp_dir().join(format!("cairn-http-remote-{}-{unique}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap_or_else(|e| panic!("mkdir {}: {e}", root.display()));
        let bare = root.join("repo.git");
        let cloned = Command::new("git")
            .args(["clone", "--quiet", "--bare"])
            .arg(source.path())
            .arg(&bare)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .status()
            .unwrap_or_else(|e| panic!("could not run git clone: {e}"));
        assert!(cloned.success(), "git clone --bare failed");

        let listener = TcpListener::bind("127.0.0.1:0").unwrap_or_else(|e| panic!("bind: {e}"));
        let addr = listener
            .local_addr()
            .unwrap_or_else(|e| panic!("local_addr: {e}"));
        let username = format!("cairn-{}-{unique}", std::process::id());
        let password = format!(
            "generated-{}-{unique}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );
        let seen = Arc::new(Mutex::new(Vec::new()));
        let stopping = Arc::new(AtomicBool::new(false));
        let serving = std::thread::spawn({
            let expected = format!(
                "Basic {}",
                base64(format!("{username}:{password}").as_bytes())
            );
            let username = username.clone();
            let root = root.clone();
            let seen = Arc::clone(&seen);
            let stopping = Arc::clone(&stopping);
            move || {
                for connection in listener.incoming() {
                    if stopping.load(Ordering::SeqCst) {
                        break;
                    }
                    let Ok(stream) = connection else { continue };
                    serve_one(stream, &root, &expected, &username, &seen);
                }
            }
        });
        Self {
            url: format!("http://{addr}/repo.git"),
            username,
            password,
            root,
            addr,
            seen,
            stopping,
            serving: Some(serving),
        }
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn username(&self) -> &str {
        &self.username
    }

    pub fn password(&self) -> &str {
        &self.password
    }

    pub fn seen(&self) -> Vec<Seen> {
        self.seen
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl Drop for HttpRemote {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.addr);
        if let Some(serving) = self.serving.take() {
            let _ = serving.join();
        }
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Answers one request and closes the connection; git reconnects per request.
fn serve_one(
    mut stream: TcpStream,
    root: &Path,
    expected_authorization: &str,
    username: &str,
    seen: &Arc<Mutex<Vec<Seen>>>,
) {
    let Some((method, target, headers)) = read_head(&mut stream) else {
        return;
    };
    let (path, query) = target.split_once('?').unwrap_or((&target, ""));
    let authorized = headers
        .get("authorization")
        .is_some_and(|given| given == expected_authorization);
    seen.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(Seen {
            path: path.to_owned(),
            authorized_as: authorized.then(|| username.to_owned()),
        });

    if headers
        .get("expect")
        .is_some_and(|e| e.eq_ignore_ascii_case("100-continue"))
    {
        let _ = stream.write_all(b"HTTP/1.1 100 Continue\r\n\r\n");
    }
    let body = read_body(&mut stream, &headers);

    if !authorized {
        let _ = stream.write_all(
            b"HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"cairn\"\r\n\
              Content-Length: 0\r\nConnection: close\r\n\r\n",
        );
        return;
    }

    let mut backend = Command::new("git");
    backend
        .arg("http-backend")
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", root)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_PROJECT_ROOT", root)
        .env("GIT_HTTP_EXPORT_ALL", "1")
        .env("GATEWAY_INTERFACE", "CGI/1.1")
        .env("SERVER_PROTOCOL", "HTTP/1.1")
        .env("REQUEST_METHOD", &method)
        .env("PATH_INFO", path)
        .env("QUERY_STRING", query)
        .env("REMOTE_ADDR", "127.0.0.1")
        .env("REMOTE_USER", username)
        .env("CONTENT_LENGTH", body.len().to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    if let Some(content_type) = headers.get("content-type") {
        backend.env("CONTENT_TYPE", content_type);
    }
    if let Some(encoding) = headers.get("content-encoding") {
        backend.env("HTTP_CONTENT_ENCODING", encoding);
    }
    if let Some(protocol) = headers.get("git-protocol") {
        backend.env("HTTP_GIT_PROTOCOL", protocol);
    }
    let Ok(mut child) = backend.spawn() else {
        let _ =
            stream.write_all(b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\n\r\n");
        return;
    };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(&body);
    }
    let Ok(output) = child.wait_with_output() else {
        return;
    };
    let (cgi_headers, cgi_body) = split_cgi(&output.stdout);
    let mut status = "200 OK".to_owned();
    let mut response = String::new();
    for line in cgi_headers.lines() {
        if let Some(rest) = line.strip_prefix("Status:") {
            status = rest.trim().to_owned();
        } else if !line.is_empty() {
            response.push_str(line);
            response.push_str("\r\n");
        }
    }
    let head = format!(
        "HTTP/1.1 {status}\r\n{response}Content-Length: {}\r\nConnection: close\r\n\r\n",
        cgi_body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(cgi_body);
    let _ = stream.flush();
}

/// The request line and headers (names lowercased); `None` for a connection
/// that sent nothing, such as the one that wakes the server to stop.
fn read_head(stream: &mut TcpStream) -> Option<(String, String, BTreeMap<String, String>)> {
    let mut bytes = Vec::new();
    let mut byte = [0u8; 1];
    while !bytes.ends_with(b"\r\n\r\n") {
        match stream.read(&mut byte) {
            Ok(1) => bytes.push(byte[0]),
            _ => return None,
        }
        if bytes.len() > 64 * 1024 {
            return None;
        }
    }
    let head = String::from_utf8_lossy(&bytes);
    let mut lines = head.lines();
    let request = lines.next()?;
    let mut parts = request.split_whitespace();
    let method = parts.next()?.to_owned();
    let target = parts.next()?.to_owned();
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_owned()))
        .collect();
    Some((method, target, headers))
}

fn read_body(stream: &mut TcpStream, headers: &BTreeMap<String, String>) -> Vec<u8> {
    if headers
        .get("transfer-encoding")
        .is_some_and(|encoding| encoding.eq_ignore_ascii_case("chunked"))
    {
        let mut body = Vec::new();
        loop {
            let size = read_line(stream);
            let size = size.split(';').next().unwrap_or("").trim();
            let Ok(size) = usize::from_str_radix(size, 16) else {
                return body;
            };
            if size == 0 {
                let _ = read_line(stream);
                return body;
            }
            let mut chunk = vec![0u8; size];
            if stream.read_exact(&mut chunk).is_err() {
                return body;
            }
            body.extend_from_slice(&chunk);
            let _ = read_line(stream);
        }
    }
    let length: usize = headers
        .get("content-length")
        .and_then(|l| l.parse().ok())
        .unwrap_or(0);
    let mut body = vec![0u8; length];
    let _ = stream.read_exact(&mut body);
    body
}

fn read_line(stream: &mut TcpStream) -> String {
    let mut line = Vec::new();
    let mut byte = [0u8; 1];
    while !line.ends_with(b"\r\n") {
        match stream.read(&mut byte) {
            Ok(1) => line.push(byte[0]),
            _ => break,
        }
    }
    String::from_utf8_lossy(&line).trim_end().to_owned()
}

/// `git http-backend` writes its headers with `\r\n` and a blank line before the body.
fn split_cgi(output: &[u8]) -> (String, &[u8]) {
    let separator = output
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .unwrap_or(output.len());
    let headers = String::from_utf8_lossy(&output[..separator]).into_owned();
    let body = output.get(separator + 4..).unwrap_or(&[]);
    (headers, body)
}

/// Standard base64 with padding, for the `Authorization` comparison.
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
    assert_eq!(base64(b""), "");
    assert_eq!(base64(b"f"), "Zg==");
    assert_eq!(base64(b"fo"), "Zm8=");
    assert_eq!(base64(b"foo"), "Zm9v");
    assert_eq!(base64(b"user:pass"), "dXNlcjpwYXNz");
}
