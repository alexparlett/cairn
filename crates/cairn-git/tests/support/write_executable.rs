//! The one way a `cairn-git` test writes a file it then has git or the engine execute (phase 19's
//! QA item 6): shared by the test binaries through `#[path]`.

/// Writes `contents` to `path` with `mode` — a stub `git`, a hook, anything a test then has git
/// or the engine execute — through a child `sh`, so this multi-threaded test process never holds
/// a writable descriptor to it. A descriptor this process held would be inherited by any child a
/// parallel test forks in that instant, and executing the file while it is open for writing
/// fails "Text file busy" (os error 26); the child's descriptor is closed when it exits, before
/// this returns. The parent directory is made first.
pub fn write_executable(path: &std::path::Path, contents: &str, mode: u32) {
    use std::io::Write as _;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .unwrap_or_else(|e| panic!("making {}: {e}", parent.display()));
    }
    let mut writer = std::process::Command::new("/bin/sh")
        .args(["-c", "cat > \"$0\" && chmod \"$1\" \"$0\""])
        .arg(path)
        .arg(format!("{mode:o}"))
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("starting the writer of {}: {e}", path.display()));
    {
        let Some(mut stdin) = writer.stdin.take() else {
            panic!("the script's writer has no stdin");
        };
        stdin
            .write_all(contents.as_bytes())
            .unwrap_or_else(|e| panic!("handing {} to its writer: {e}", path.display()));
    }
    let status = writer
        .wait()
        .unwrap_or_else(|e| panic!("waiting on the writer of {}: {e}", path.display()));
    assert!(
        status.success(),
        "writing {} failed: {status}",
        path.display()
    );
}
