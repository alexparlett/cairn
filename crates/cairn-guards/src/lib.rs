//! Matchers and repository walking for Cairn's invariant guards.
//!
//! The assertions live in `tests/invariants.rs`; this crate holds the pieces
//! they are built from, each with a unit test proving it fires on the DISGUISED
//! forms of what it forbids. A matcher that only catches the obvious spelling
//! reports green while the invariant rots.

use std::path::{Path, PathBuf};

/// The workspace root, resolved from this crate's manifest rather than the
/// process working directory so the guards run the same under `cargo test`,
/// the gate, and CI.
pub fn repo_root() -> PathBuf {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    match manifest_dir.ancestors().nth(2) {
        Some(root) => root.to_path_buf(),
        None => panic!(
            "cairn-guards should sit two directories below the workspace root, but {} has no such ancestor",
            manifest_dir.display()
        ),
    }
}

/// Every `.rs` file under `dir`, as (path relative to the repo root, source).
///
/// Panics when the walk finds nothing: a guard pointed at a directory that was
/// renamed must fail, not pass forever over an empty set.
pub fn rust_sources(dir: impl AsRef<Path>) -> Vec<(PathBuf, String)> {
    let root = repo_root();
    let dir = root.join(dir.as_ref());
    let mut found = Vec::new();
    collect_rust(&dir, &mut found);
    assert!(
        !found.is_empty(),
        "guard walked {} and found no .rs files; if the code moved, move the guard",
        dir.display()
    );
    found
        .into_iter()
        .map(|path| {
            let source = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
            let relative = path.strip_prefix(&root).unwrap_or(&path).to_path_buf();
            (relative, source)
        })
        .collect()
}

fn collect_rust(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n == "target") {
                continue;
            }
            collect_rust(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// `source` with comments blanked out, line structure preserved.
///
/// Guards match against this view so prose that NAMES a forbidden thing (this
/// file does it constantly) is not a violation, while string literals — which
/// is how a subprocess names `git` — still are.
pub fn code_only(source: &str) -> String {
    #[derive(Clone, Copy)]
    enum Mode {
        Code,
        LineComment,
        BlockComment(usize),
        Str,
        RawStr(usize),
        Char,
    }

    let bytes = source.as_bytes();
    let mut out = String::with_capacity(source.len());
    let mut mode = Mode::Code;
    let mut i = 0;

    while i < bytes.len() {
        let b = bytes[i];
        let next = bytes.get(i + 1).copied();
        match mode {
            Mode::Code => match (b, next) {
                (b'/', Some(b'/')) => {
                    mode = Mode::LineComment;
                    out.push_str("  ");
                    i += 2;
                }
                (b'/', Some(b'*')) => {
                    mode = Mode::BlockComment(1);
                    out.push_str("  ");
                    i += 2;
                }
                (b'"', _) => {
                    mode = Mode::Str;
                    out.push('"');
                    i += 1;
                }
                (b'\'', _) => {
                    mode = Mode::Char;
                    out.push('\'');
                    i += 1;
                }
                (b'r', Some(b'"' | b'#')) => {
                    let hashes = bytes[i + 1..].iter().take_while(|&&c| c == b'#').count();
                    if bytes.get(i + 1 + hashes) == Some(&b'"') {
                        mode = Mode::RawStr(hashes);
                        out.push_str(&source[i..i + hashes + 2]);
                        i += hashes + 2;
                    } else {
                        out.push('r');
                        i += 1;
                    }
                }
                _ => {
                    out.push(b as char);
                    i += 1;
                }
            },
            Mode::LineComment => {
                if b == b'\n' {
                    mode = Mode::Code;
                    out.push('\n');
                } else {
                    out.push(' ');
                }
                i += 1;
            }
            Mode::BlockComment(depth) => match (b, next) {
                (b'/', Some(b'*')) => {
                    mode = Mode::BlockComment(depth + 1);
                    out.push_str("  ");
                    i += 2;
                }
                (b'*', Some(b'/')) => {
                    mode = if depth == 1 {
                        Mode::Code
                    } else {
                        Mode::BlockComment(depth - 1)
                    };
                    out.push_str("  ");
                    i += 2;
                }
                _ => {
                    out.push(if b == b'\n' { '\n' } else { ' ' });
                    i += 1;
                }
            },
            Mode::Str | Mode::Char => {
                let closer = if matches!(mode, Mode::Str) {
                    b'"'
                } else {
                    b'\''
                };
                if b == b'\\' {
                    out.push('\\');
                    if let Some(n) = next {
                        out.push(n as char);
                    }
                    i += 2;
                } else {
                    if b == closer {
                        mode = Mode::Code;
                    }
                    out.push(b as char);
                    i += 1;
                }
            }
            Mode::RawStr(hashes) => {
                if b == b'"' && bytes[i + 1..].iter().take_while(|&&c| c == b'#').count() >= hashes
                {
                    mode = Mode::Code;
                    out.push_str(&source[i..i + hashes + 1]);
                    i += hashes + 1;
                } else {
                    out.push(b as char);
                    i += 1;
                }
            }
        }
    }
    out
}

/// [`code_only`], with the CONTENTS of double-quoted strings blanked too.
///
/// `spawns_git` needs string literals, because `"git"` is how a subprocess
/// names its program. A matcher looking for waiting primitives does not: a
/// status line reading "waiting to receive" is prose that happens to live in a
/// string, and a guard that reddens on it is a guard somebody switches off.
/// Char literals are RECOGNISED and stepped over, contents blanked: `'"'` is a
/// legal char literal, and a scanner that misreads its quote as the start of a
/// string blanks everything up to the next `"` in the file, taking both guards
/// built on this dark for the whole file. Recognition is exact rather than a
/// forward search for a closing apostrophe; `char_literal_end` states the rule.
pub fn code_without_strings(source: &str) -> String {
    let code = code_only(source);
    let bytes = code.as_bytes();
    let mut out = String::with_capacity(code.len());
    let mut i = 0;

    while i < bytes.len() {
        // A raw string: r, some hashes, a quote. Ends at a quote followed by as
        // many hashes.
        let raw_hashes = if bytes[i] == b'r' {
            let hashes = bytes[i + 1..].iter().take_while(|&&c| c == b'#').count();
            (bytes.get(i + 1 + hashes) == Some(&b'"')).then_some(hashes)
        } else {
            None
        };
        if let Some(hashes) = raw_hashes {
            out.push_str(&code[i..i + hashes + 2]);
            i += hashes + 2;
            while i < bytes.len() {
                if bytes[i] == b'"'
                    && bytes[i + 1..].iter().take_while(|&&c| c == b'#').count() >= hashes
                {
                    out.push_str(&code[i..i + hashes + 1]);
                    i += hashes + 1;
                    break;
                }
                out.push(if bytes[i] == b'\n' { '\n' } else { ' ' });
                i += 1;
            }
            continue;
        }
        if bytes[i] == b'"' {
            out.push('"');
            i += 1;
            while i < bytes.len() {
                if bytes[i] == b'\\' {
                    out.push_str("  ");
                    i += 2;
                    continue;
                }
                if bytes[i] == b'"' {
                    out.push('"');
                    i += 1;
                    break;
                }
                out.push(if bytes[i] == b'\n' { '\n' } else { ' ' });
                i += 1;
            }
            continue;
        }
        // A lifetime reaches neither arm: `char_literal_end` returns `None`,
        // and the apostrophe falls through to be emitted as itself.
        if bytes[i] == b'\''
            && let Some(end) = char_literal_end(bytes, i)
        {
            out.push('\'');
            for _ in i + 1..end {
                out.push(' ');
            }
            out.push('\'');
            i = end + 1;
            continue;
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

/// The index of the closing apostrophe of the char literal starting at `open`,
/// or `None` when that apostrophe opens a lifetime instead.
///
/// Exact by construction: a char literal is an apostrophe, then either a
/// backslash escape or exactly one character, then the closing apostrophe.
/// Anything else is a lifetime, so `&'a str` is left alone.
fn char_literal_end(bytes: &[u8], open: usize) -> Option<usize> {
    let after = open + 1;
    if bytes.get(after) == Some(&b'\\') {
        // `'\''` is the case that makes a naive "next apostrophe" search wrong
        // in the other direction: the escaped one is not the closing one.
        let mut end = after + 2;
        while bytes.get(end).is_some_and(|c| *c != b'\'' && *c != b'\n') {
            end += 1;
        }
        return (bytes.get(end) == Some(&b'\'')).then_some(end);
    }
    // One character, which may be several bytes: step over the continuation
    // bytes of a UTF-8 sequence.
    let mut end = after + 1;
    while bytes
        .get(end)
        .is_some_and(|c| c & 0b1100_0000 == 0b1000_0000)
    {
        end += 1;
    }
    (bytes.get(end) == Some(&b'\'')).then_some(end)
}

/// [`code_without_strings`] output with `#[cfg(test)]` modules blanked.
///
/// For the checks that ask what a file DOES rather than what it must not do. A
/// requirement satisfied from a test module is not satisfied: "production
/// switched to a hand-rolled viewport while a test still names the virtualizing
/// view" passes green against a whole-file token search.
///
/// Brace counting is honest here only because it runs on
/// [`code_without_strings`] output, where comments, string literals and char
/// literals have already been blanked — so every brace left is a real one. Line
/// numbers are preserved, because callers report them.
pub fn code_without_test_modules(code: &str) -> String {
    const MARKER: &[u8] = b"#[cfg(test)]";
    let bytes = code.as_bytes();
    let mut out: Vec<u8> = bytes.to_vec();
    let mut i = 0;

    while i + MARKER.len() <= bytes.len() {
        if &bytes[i..i + MARKER.len()] != MARKER {
            i += 1;
            continue;
        }
        // The module's opening brace, if this attribute introduces a block at
        // all: `#[cfg(test)] mod tests;` and `#[cfg(test)] use x;` declare no
        // body, so a `;` first means there is nothing to blank.
        let mut open = i + MARKER.len();
        while open < bytes.len() && bytes[open] != b'{' && bytes[open] != b';' {
            open += 1;
        }
        if bytes.get(open) != Some(&b'{') {
            i += MARKER.len();
            continue;
        }

        let mut depth = 0usize;
        let mut end = open;
        while end < bytes.len() {
            match bytes[end] {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
            end += 1;
        }
        // An unbalanced file ends the blanking at its end rather than panicking:
        // the guard reports on what it could read, and the compiler has a much
        // better complaint about the braces.
        let end = end.min(bytes.len().saturating_sub(1));

        for byte in &mut out[i..=end] {
            if *byte != b'\n' {
                *byte = b' ';
            }
        }
        i = end + 1;
    }

    String::from_utf8(out).unwrap_or_default()
}

/// 1-based line numbers where `source` names the crate `ident` as a path root
/// or imports it, in code (not prose).
///
/// Catches `use gix::x`, `use gix as g`, `gix::open`, `::gix::open`,
/// `<crate>::gix` re-exports, and spacing variants — anything where the
/// identifier appears with word boundaries outside a comment.
pub fn mentions_crate(source: &str, ident: &str) -> Vec<usize> {
    let code = code_only(source);
    let mut hits = Vec::new();
    for (n, line) in code.lines().enumerate() {
        if line_has_ident(line, ident) {
            hits.push(n + 1);
        }
    }
    hits
}

fn line_has_ident(line: &str, ident: &str) -> bool {
    !ident_offsets(line, ident).is_empty()
}

/// Byte offsets where `ident` appears in `text` with word boundaries either
/// side. Offsets rather than lines, because a matcher may need to look at what
/// FOLLOWS the identifier, and what follows can be on the next line.
fn ident_offsets(text: &str, ident: &str) -> Vec<usize> {
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(offset) = text[from..].find(ident) {
        let start = from + offset;
        let end = start + ident.len();
        let before_ok = start == 0 || !is_ident_byte(bytes[start - 1]);
        let after_ok = end == bytes.len() || !is_ident_byte(bytes[end]);
        if before_ok && after_ok {
            found.push(start);
        }
        from = end;
    }
    found
}

/// The 1-based line `offset` falls on.
fn line_at(text: &str, offset: usize) -> usize {
    text[..offset].bytes().filter(|b| *b == b'\n').count() + 1
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Identifiers whose presence means the code can wait for something.
///
/// Naming one is enough — you cannot alias what you have not first named, so
/// `use std::sync::mpsc::Receiver as Rx` is caught on its import line even
/// though every later use spells it `Rx`.
const WAITING_IDENTS: &[&str] = &[
    "Barrier",
    "Condvar",
    "JoinHandle",
    "Mutex",
    "Receiver",
    "RwLock",
    // The constructors, because a receiver can be held without ever naming its
    // type: `let (tx, rx) = channel();` then `for update in rx {}` blocks, and
    // spells nothing on the roster above. You cannot have a channel without
    // building one.
    "bounded",
    "channel",
    "sync_channel",
    "unbounded",
    "block_on",
    "blocking_lock",
    "blocking_recv",
    "blocking_send",
    "park",
    "park_timeout",
    "recv_deadline",
    "recv_timeout",
    "scope",
    "select",
    "sleep",
    "wait_timeout",
    "wait_while",
    // These do not block, they spin — which costs a UI thread the same core.
    // Only the worker module has reason to name one, and it is exempt.
    "spin_loop",
    "try_iter",
    "try_lock",
    "try_recv",
    "yield_now",
];

/// Methods that wait when called with NO arguments.
///
/// Separated from the list above because each has an innocent namesake that
/// takes one: `Path::join("crates")` and `[..].join(", ")` are not waits.
const WAITING_NULLARY_CALLS: &[&str] = &["join", "lock", "recv", "wait"];

/// 1-based line numbers where `source` waits for something, in code.
///
/// The invariant this serves is "the UI thread never waits on repository work".
/// A render path has no legitimate reason to name any of these, so the matcher
/// forbids the spellings rather than trying to decide what is being waited FOR,
/// which is not decidable from source.
pub fn waits_on_work(source: &str) -> Vec<usize> {
    let code = code_without_strings(source);
    let mut lines = std::collections::BTreeSet::new();

    for ident in WAITING_IDENTS {
        for offset in ident_offsets(&code, ident) {
            lines.insert(line_at(&code, offset));
        }
    }
    for name in WAITING_NULLARY_CALLS {
        for offset in ident_offsets(&code, name) {
            if takes_no_arguments(&code, offset + name.len()) {
                lines.insert(line_at(&code, offset));
            }
        }
    }
    lines.into_iter().collect()
}

/// Whether what follows `at` is `()`, with any amount of whitespace — including
/// newlines — inside and before it.
fn takes_no_arguments(code: &str, at: usize) -> bool {
    let mut rest = code[at..].trim_start().chars();
    if rest.next() != Some('(') {
        return false;
    }
    rest.as_str().trim_start().starts_with(')')
}

/// 1-based line numbers where `source` spawns a `git` subprocess, in code.
///
/// Matches the `Command` spelling and its aliases by looking for the literal
/// program name rather than the constructor, so `process::Command::new("git")`,
/// `Command::new("git")` and `cmd("git")` helpers all land.
pub fn spawns_git(source: &str) -> Vec<usize> {
    let code = code_only(source);
    code.lines()
        .enumerate()
        .filter(|(_, line)| {
            let l = line.trim();
            (l.contains("\"git\"") || l.contains("\"/usr/bin/git\""))
                && (l.contains("Command") || l.contains("new(") || l.contains("cmd("))
        })
        .map(|(n, _)| n + 1)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_only_blanks_comments_and_keeps_strings() {
        let src = "let a = 1; // gix::open\n/* gix */ let b = \"gix::open\";\n";
        let code = code_only(src);
        assert!(
            !code.contains("gix::open\n"),
            "line comment survived: {code:?}"
        );
        assert!(
            code.contains("\"gix::open\""),
            "string literal was eaten: {code:?}"
        );
        assert_eq!(code.lines().count(), src.lines().count());
    }

    #[test]
    fn code_only_handles_nested_blocks_and_raw_strings() {
        let code = code_only("/* outer /* inner */ still */ let s = r#\"gix\"#;");
        assert!(!code.contains("inner"));
        assert!(code.contains("r#\"gix\"#"));
    }

    #[test]
    fn code_only_does_not_treat_a_slash_inside_a_string_as_a_comment() {
        let code = code_only("let p = \"a // b\"; let q = 1;");
        assert!(code.contains("let q = 1;"), "{code:?}");
    }

    #[test]
    fn mentions_crate_catches_the_disguised_forms() {
        for (label, src) in [
            ("plain path", "gix::open(p)"),
            ("leading colons", "::gix::open(p)"),
            ("aliased import", "use gix as backend;"),
            ("grouped import", "use {std::fs, gix};"),
            ("extern crate", "extern crate gix;"),
            ("wrapped line", "let r =\n    gix::discover(p)?;"),
            ("spaced path", "gix :: open(p)"),
        ] {
            assert!(
                !mentions_crate(src, "gix").is_empty(),
                "missed the {label} form"
            );
        }
    }

    #[test]
    fn mentions_crate_ignores_prose_and_longer_identifiers() {
        assert!(mentions_crate("// gix::open is forbidden here\n", "gix").is_empty());
        assert!(mentions_crate("//! we do not use gix\n", "gix").is_empty());
        assert!(mentions_crate("let gixture = 1;", "gix").is_empty());
        assert!(mentions_crate("use my_gix::thing;", "gix").is_empty());
    }

    /// Every entry in both rosters, spelled out as a literal here rather than
    /// looped over the roster itself: looping would shrink with the roster and
    /// stay green while the guard lost half its coverage. A fixed list means
    /// deleting an entry turns this red.
    #[test]
    fn every_waiting_spelling_in_the_roster_is_matched() {
        let spellings = [
            ("Barrier", "let gate = Barrier::new(2);"),
            ("Condvar", "let ready = Condvar::new();"),
            ("JoinHandle", "let h: JoinHandle<()> = spawn(f);"),
            ("Mutex", "let held = Mutex::new(1);"),
            ("Receiver", "use std::sync::mpsc::Receiver;"),
            ("RwLock", "let shared = RwLock::new(1);"),
            ("bounded", "let (tx, rx) = bounded(8);"),
            ("channel", "let (tx, rx) = std::sync::mpsc::channel();"),
            ("sync_channel", "let (tx, rx) = sync_channel(1);"),
            (
                "unbounded",
                "let (tx, rx) = crossbeam_channel::unbounded();",
            ),
            ("block_on", "futures::executor::block_on(fut);"),
            ("blocking_lock", "let held = shared.blocking_lock();"),
            ("blocking_recv", "let next = rx.blocking_recv();"),
            ("blocking_send", "tx.blocking_send(v)?;"),
            ("park", "std::thread::park();"),
            ("park_timeout", "std::thread::park_timeout(d);"),
            ("recv_deadline", "rx.recv_deadline(at)?;"),
            ("recv_timeout", "rx.recv_timeout(d)?;"),
            ("scope", "std::thread::scope(|s| s.spawn(f));"),
            (
                "select",
                "crossbeam_channel::select! { recv(rx) -> v => {} }",
            ),
            ("sleep", "std::thread::sleep(d);"),
            ("wait_timeout", "let (g, r) = cv.wait_timeout(g, d)?;"),
            ("wait_while", "let g = cv.wait_while(g, |s| !s.ready)?;"),
            (
                "spin_loop",
                "while !done.load(Acquire) { std::hint::spin_loop(); }",
            ),
            ("try_iter", "for update in rx.try_iter() {}"),
            ("try_lock", "if let Ok(g) = shared.try_lock() {}"),
            ("try_recv", "while let Ok(u) = rx.try_recv() {}"),
            ("yield_now", "std::thread::yield_now();"),
            ("join", "handle.join().unwrap();"),
            ("lock", "let held = shared.lock();"),
            ("recv", "let next = rx.recv();"),
            ("wait", "let g = cv.wait();"),
        ];
        for (spelling, src) in spellings {
            assert!(
                !waits_on_work(src).is_empty(),
                "`{spelling}` is no longer matched: {src:?}"
            );
        }
        let covered: std::collections::BTreeSet<&str> =
            spellings.iter().map(|(name, _)| *name).collect();
        for entry in WAITING_IDENTS.iter().chain(WAITING_NULLARY_CALLS) {
            assert!(
                covered.contains(entry),
                "`{entry}` was added to a roster without a line in this test"
            );
        }
    }

    #[test]
    fn waits_on_work_catches_the_disguised_forms() {
        for (label, src) in [
            ("plain receive", "let next = rx.recv();"),
            ("wrapped call", "let next = rx\n    .recv()\n    .ok();"),
            ("spaced parens", "let next = rx.recv ();"),
            ("newline inside the parens", "let next = rx.recv(\n);"),
            ("aliased import", "use std::sync::mpsc::Receiver as Rx;"),
            ("grouped import", "use std::sync::{Arc, Mutex};"),
            ("qualified path", "let g = std::sync::Mutex::new(1);"),
            (
                "inferred receiver",
                "let (tx, rx) = std::sync::mpsc::channel();",
            ),
            (
                "iterating a receiver",
                "let (tx, rx) = channel();\nfor u in rx {}",
            ),
            ("renamed sleep", "use std::thread::sleep as nap;"),
        ] {
            assert!(
                !waits_on_work(src).is_empty(),
                "missed the {label} form: {src:?}"
            );
        }
    }

    #[test]
    fn waits_on_work_ignores_prose_and_innocent_namesakes() {
        assert!(waits_on_work("// rx.recv() is forbidden on this side\n").is_empty());
        assert!(waits_on_work("//! nothing here may call join() either\n").is_empty());
        assert!(waits_on_work("let p = root.join(\"crates\");").is_empty());
        assert!(waits_on_work("let s = parts.join(\", \");").is_empty());
        assert!(waits_on_work("let joined = 1; let received = 2;").is_empty());
        assert!(waits_on_work("let v = signal.read(); v.write();").is_empty());
        assert!(waits_on_work("use my_crate::Receivers;").is_empty());
        // Prose inside a string literal is still prose. A status line a user
        // reads must be free to say what the code is doing.
        assert!(waits_on_work("status.set(\"waiting to receive a lock\");").is_empty());
        assert!(waits_on_work("let hint = r#\"sleep until the Mutex frees\"#;").is_empty());
    }

    /// The fail-QUIET case: a char literal holding a double quote must not open
    /// a blanking run that eats the rest of the file, or a guard built on
    /// [`code_without_strings`] reports green over source it never read. Every
    /// form of it, and a lifetime beside each, which must not be blanked.
    #[test]
    fn a_quote_inside_a_char_literal_does_not_blank_the_rest_of_the_file() {
        for quote in ["'\"'", "b'\"'", "'\\\"'"] {
            let src = format!("fn q() -> char {{ {quote} }}\nlet c = rx.recv();\n");
            assert_eq!(
                waits_on_work(&src),
                vec![2],
                "{quote} blanked the code after it"
            );
            assert_eq!(
                mentions_crate(&code_without_strings(&src), "rx"),
                vec![2],
                "{quote} hid an identifier after it"
            );
        }
    }

    /// The other direction: a lifetime is not a literal, and blanking from one
    /// apostrophe to the next would eat the code between two of them.
    #[test]
    fn a_lifetime_is_not_mistaken_for_a_char_literal() {
        let src = "fn f<'a, 'b>(x: &'a str, y: &'b Mutex) { let _ = x.recv(); }";
        assert_eq!(
            waits_on_work(src),
            vec![1],
            "a lifetime pair blanked the code between them"
        );
        // And the escaped-apostrophe literal, whose closing quote is the third
        // apostrophe rather than the second.
        let src = "let tick = '\\''; let _ = rx.recv();";
        assert_eq!(waits_on_work(src), vec![1]);
    }

    /// A requirement met from a test module is not met. Both directions, and
    /// the line numbers the callers report must survive the blanking.
    #[test]
    fn a_test_module_is_not_part_of_what_a_file_does() {
        let src = "\
use freya::prelude::*;
fn render() -> Element {
    hand_rolled_viewport()
}
#[cfg(test)]
mod tests {
    use super::*;
    fn it() {
        let _ = VirtualScrollView::new();
        let nested = || { 1 };
    }
}
";
        let code = code_without_test_modules(&code_without_strings(src));
        assert!(
            mentions_crate(&code, "VirtualScrollView").is_empty(),
            "a token inside a test module counted as something the file does"
        );
        assert_eq!(
            mentions_crate(&code, "freya"),
            vec![1],
            "blanking the test module moved or lost the production lines"
        );

        // The same token in production is still seen, so the blanking is
        // scoped rather than a way to turn the check off.
        let production = code_without_test_modules(&code_without_strings(
            "fn render() { VirtualScrollView::new(); }\n#[cfg(test)]\nmod tests { fn t() {} }\n",
        ));
        assert_eq!(mentions_crate(&production, "VirtualScrollView"), vec![1]);

        // A `#[cfg(test)]` with no body blanks nothing after it.
        let no_body = code_without_test_modules(&code_without_strings(
            "#[cfg(test)]\nmod tests;\nfn render() { VirtualScrollView::new(); }\n",
        ));
        assert_eq!(mentions_crate(&no_body, "VirtualScrollView"), vec![3]);
    }

    #[test]
    fn waits_on_work_reports_the_line_the_wait_is_on() {
        let src = "let a = 1;\nlet b = 2;\nlet c = rx.recv();\n";
        assert_eq!(waits_on_work(src), vec![3]);
    }

    #[test]
    fn spawns_git_catches_qualified_and_aliased_constructors() {
        for src in [
            "Command::new(\"git\")",
            "std::process::Command::new(\"git\")",
            "let c = process::Command::new(\"git\");",
            "cmd(\"git\").arg(\"status\")",
        ] {
            assert!(!spawns_git(src).is_empty(), "missed: {src}");
        }
    }

    #[test]
    fn spawns_git_ignores_prose_and_unrelated_strings() {
        assert!(spawns_git("// Command::new(\"git\") is forbidden\n").is_empty());
        assert!(spawns_git("let msg = \"git is not installed\";").is_empty());
    }

    #[test]
    fn rust_sources_finds_this_crate() {
        let files = rust_sources("crates/cairn-guards/src");
        assert!(files.iter().any(|(p, _)| p.ends_with("lib.rs")));
    }

    #[test]
    #[should_panic(expected = "found no .rs files")]
    fn rust_sources_fails_loudly_on_a_moved_directory() {
        rust_sources("crates/cairn-this-crate-does-not-exist/src");
    }
}
