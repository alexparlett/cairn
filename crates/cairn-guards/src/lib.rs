//! Matchers and repository walking for the invariant guards.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Resolved from this crate's manifest, not the process working directory.
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

/// Every `.rs` file under `dir`, as (path relative to the repo root, source). Panics on an empty walk.
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

/// The crates a manifest declares, by package name (a `package = ".."` rename is seen through).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct DeclaredDependencies {
    /// `[dependencies]` and `[build-dependencies]`, including `[target.*]` tables.
    pub shipped: BTreeSet<String>,
    /// `[dev-dependencies]`, including `[target.*]` tables.
    pub test_only: BTreeSet<String>,
}

pub fn declared_dependencies(manifest: &toml::Table) -> DeclaredDependencies {
    let mut declared = DeclaredDependencies::default();
    let mut scopes = vec![manifest];
    if let Some(targets) = manifest.get("target").and_then(toml::Value::as_table) {
        scopes.extend(targets.values().filter_map(toml::Value::as_table));
    }
    for scope in scopes {
        for (table, test_only) in [
            ("dependencies", false),
            ("build-dependencies", false),
            ("dev-dependencies", true),
        ] {
            let Some(entries) = scope.get(table).and_then(toml::Value::as_table) else {
                continue;
            };
            for (key, spec) in entries {
                let name = spec
                    .get("package")
                    .and_then(toml::Value::as_str)
                    .unwrap_or(key)
                    .to_owned();
                if test_only {
                    declared.test_only.insert(name);
                } else {
                    declared.shipped.insert(name);
                }
            }
        }
    }
    declared
}

/// `source` with comments blanked, line structure preserved; string literals are kept.
pub fn code_only(source: &str) -> String {
    #[derive(Clone, Copy)]
    enum Mode {
        Code,
        LineComment,
        BlockComment(usize),
        Str,
        RawStr(usize),
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
                // A char literal is kept whole; a lifetime's apostrophe is just an
                // apostrophe. Treating every apostrophe as opening a literal once
                // blanked a file from a `&'static str` to the next `'` inside a
                // string, and every matcher after that point saw nothing.
                (b'\'', _) => match char_literal_end(bytes, i) {
                    Some(end) => {
                        out.push_str(&source[i..=end]);
                        i = end + 1;
                    }
                    None => {
                        out.push('\'');
                        i += 1;
                    }
                },
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
            Mode::Str => {
                if b == b'\\' {
                    out.push('\\');
                    if let Some(n) = next {
                        out.push(n as char);
                    }
                    i += 2;
                } else {
                    if b == b'"' {
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

/// [`code_only`], with the contents of double-quoted strings blanked too. Char literals
/// are stepped over exactly, so `'"'` does not open a string.
pub fn code_without_strings(source: &str) -> String {
    let code = code_only(source);
    let bytes = code.as_bytes();
    let mut out = String::with_capacity(code.len());
    let mut i = 0;

    while i < bytes.len() {
        // A raw string: r, hashes, quote. Ends at a quote plus as many hashes.
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
                    // An escaped newline continues the string; the line count must not drop.
                    out.push(' ');
                    out.push(if bytes.get(i + 1) == Some(&b'\n') {
                        '\n'
                    } else {
                        ' '
                    });
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
        // A lifetime reaches neither arm: `char_literal_end` returns `None` and
        // the apostrophe is emitted as itself.
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

/// The closing apostrophe of the char literal starting at `open`, or `None` for a lifetime.
fn char_literal_end(bytes: &[u8], open: usize) -> Option<usize> {
    let after = open + 1;
    if bytes.get(after) == Some(&b'\\') {
        // `'\''`: the escaped apostrophe is not the closing one.
        let mut end = after + 2;
        while bytes.get(end).is_some_and(|c| *c != b'\'' && *c != b'\n') {
            end += 1;
        }
        return (bytes.get(end) == Some(&b'\'')).then_some(end);
    }
    // One character, possibly several bytes: step over UTF-8 continuations.
    let mut end = after + 1;
    while bytes
        .get(end)
        .is_some_and(|c| c & 0b1100_0000 == 0b1000_0000)
    {
        end += 1;
    }
    (bytes.get(end) == Some(&b'\'')).then_some(end)
}

/// [`code_without_strings`] output with `#[cfg(test)]` modules blanked, line numbers kept.
/// Brace counting is sound only on [`code_without_strings`] output.
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
        // The module's opening brace; a `;` first (`mod tests;`) means there is nothing to blank.
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
        // An unbalanced file ends the blanking at its end rather than panicking.
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

/// 1-based lines where `source` names the crate `ident` as a path root or import, in code,
/// including aliases, `::ident` and re-exports.
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

/// Byte offsets where `ident` appears in `text` with word boundaries either side.
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

/// Identifiers whose presence means the code can wait; an alias is caught on its import line.
const WAITING_IDENTS: &[&str] = &[
    "Barrier",
    "Condvar",
    "JoinHandle",
    "Mutex",
    "Receiver",
    "RwLock",
    // Constructors: `for update in rx {}` blocks without naming anything above.
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
    // These spin rather than block.
    "spin_loop",
    "try_iter",
    "try_lock",
    "try_recv",
    "yield_now",
];

/// Methods that wait when called with no arguments; each has an innocent one-argument namesake.
const WAITING_NULLARY_CALLS: &[&str] = &["join", "lock", "recv", "wait"];

/// 1-based lines where `source` names a way to wait, in code.
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

/// Whether what follows `at` is `()`, allowing whitespace and newlines.
fn takes_no_arguments(code: &str, at: usize) -> bool {
    let mut rest = code[at..].trim_start().chars();
    if rest.next() != Some('(') {
        return false;
    }
    rest.as_str().trim_start().starts_with(')')
}

const ROW_CONTENT: &str = "RowContent";

/// 1-based lines where `source` reads a `RowContent` without naming every variant: a wildcard or
/// catch-all arm in a match that names it (including `Some(_)` beside `Some(RowContent::..)`),
/// `if let`/`while let`/let-chain/`let .. else` over it, `matches!` over it, or an import of
/// its variants or of it under another name.
pub fn reads_row_content_partially(source: &str) -> Vec<usize> {
    let code = code_without_strings(source);
    let bytes = code.as_bytes();
    let names_row_content = |text: &str| !ident_offsets(text, ROW_CONTENT).is_empty();
    let mut lines = BTreeSet::new();

    for offset in ident_offsets(&code, ROW_CONTENT) {
        let rest = code[offset + ROW_CONTENT.len()..].trim_start();
        let glob = rest
            .strip_prefix("::")
            .is_some_and(|r| r.trim_start().starts_with('*'));
        // Past this, a view can name a variant without spelling `RowContent`.
        let renamed_or_split = in_use_statement(&code, offset)
            && (rest.starts_with("::")
                || rest
                    .strip_prefix("as")
                    .is_some_and(|r| r.starts_with(char::is_whitespace)));
        if glob || renamed_or_split {
            lines.insert(line_at(&code, offset));
        }
    }

    for offset in ident_offsets(&code, "matches") {
        let mut at = skip_whitespace(bytes, offset + "matches".len());
        if bytes.get(at) != Some(&b'!') {
            continue;
        }
        at = skip_whitespace(bytes, at + 1);
        if !matches!(bytes.get(at), Some(b'(' | b'[' | b'{')) {
            continue;
        }
        let end = balanced_end(bytes, at);
        if names_row_content(&code[at..end]) {
            lines.insert(line_at(&code, offset));
        }
    }

    for offset in ident_offsets(&code, "let") {
        let Some(assign) = depth_zero_assignment(bytes, offset + "let".len()) else {
            continue;
        };
        if !names_row_content(&code[offset..assign]) {
            continue;
        }
        let before = code[..offset].trim_end();
        let conditional = before.ends_with("&&")
            || ["if", "while"].iter().any(|keyword| {
                before.ends_with(keyword)
                    && before[..before.len() - keyword.len()]
                        .bytes()
                        .next_back()
                        .is_none_or(|b| !is_ident_byte(b))
            });
        if conditional || has_else_before_semicolon(&code, assign + 1) {
            lines.insert(line_at(&code, offset));
        }
    }

    for offset in ident_offsets(&code, "match") {
        let Some(open) = match_arms_open(bytes, offset + "match".len()) else {
            continue;
        };
        let arms = match_arm_patterns(&code, open);
        if !arms.iter().any(|(_, pattern)| names_row_content(pattern)) {
            continue;
        }
        let wrappers: BTreeSet<&str> = arms
            .iter()
            .flat_map(|(_, pattern)| split_depth_zero(strip_guard(pattern), b'|'))
            .filter_map(|alternative| wrapped(alternative))
            .filter(|(_, inner)| names_row_content(inner))
            .map(|(head, _)| head)
            .collect();
        for (at, pattern) in arms {
            if split_depth_zero(strip_guard(pattern), b'|')
                .into_iter()
                .any(|alternative| {
                    is_catch_all(alternative)
                        || wrapped(alternative).is_some_and(|(head, inner)| {
                            wrappers.contains(head)
                                && split_depth_zero(inner, b',').into_iter().all(is_catch_all)
                        })
                })
            {
                lines.insert(line_at(&code, at));
            }
        }
    }

    lines.into_iter().collect()
}

/// Whether `offset` sits inside a `use` item: the keyword appears since the last `;`.
fn in_use_statement(code: &str, offset: usize) -> bool {
    let start = code[..offset].rfind(';').map_or(0, |at| at + 1);
    !ident_offsets(&code[start..offset], "use").is_empty()
}

/// `Head(inner)` as (`Head`, `inner`), for a pattern that is one wrapper and nothing else.
fn wrapped(alternative: &str) -> Option<(&str, &str)> {
    let pattern = strip_reference(strip_attributes(alternative.trim()));
    let open = pattern.find('(')?;
    let head = pattern[..open].trim();
    let is_path = !head.is_empty() && head.bytes().all(|b| is_ident_byte(b) || b == b':');
    let end = balanced_end(pattern.as_bytes(), open);
    (is_path && end == pattern.len()).then(|| (head, &pattern[open + 1..end - 1]))
}

fn strip_attributes(mut pattern: &str) -> &str {
    while pattern.starts_with("#[") {
        let end = balanced_end(pattern.as_bytes(), 1);
        pattern = pattern[end..].trim_start();
    }
    pattern
}

/// `pattern` without a leading `&` or `&mut`.
fn strip_reference(pattern: &str) -> &str {
    match pattern.strip_prefix('&') {
        Some(rest) => {
            let rest = rest.trim_start();
            rest.strip_prefix("mut ").map_or(rest, str::trim_start)
        }
        None => pattern,
    }
}

fn skip_whitespace(bytes: &[u8], mut at: usize) -> usize {
    while bytes.get(at).is_some_and(u8::is_ascii_whitespace) {
        at += 1;
    }
    at
}

/// One past the bracket that closes the one at `open`, or the end of `bytes`.
fn balanced_end(bytes: &[u8], open: usize) -> usize {
    let mut depth = 0usize;
    for (i, b) in bytes.iter().enumerate().skip(open) {
        match b {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return i + 1;
                }
            }
            _ => {}
        }
    }
    bytes.len()
}

/// The `=` of a `let` starting at `from`: not `==`, `=>`, `!=`, `<=` or `>=`, and outside brackets.
fn depth_zero_assignment(bytes: &[u8], from: usize) -> Option<usize> {
    let mut depth = 0usize;
    for i in from..bytes.len() {
        match bytes[i] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => {
                if depth == 0 {
                    return None;
                }
                depth -= 1;
            }
            b';' if depth == 0 => return None,
            b'=' if depth == 0 => {
                let next = bytes.get(i + 1).copied();
                let prev = i.checked_sub(1).map(|p| bytes[p]);
                if !matches!(next, Some(b'=' | b'>'))
                    && !matches!(prev, Some(b'=' | b'!' | b'<' | b'>'))
                {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Whether an `else` keyword comes before the statement's `;`, outside brackets.
fn has_else_before_semicolon(code: &str, from: usize) -> bool {
    let bytes = code.as_bytes();
    let mut depth = 0usize;
    let mut i = from;
    while i < bytes.len() {
        match bytes[i] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => {
                if depth == 0 {
                    return false;
                }
                depth -= 1;
            }
            b';' if depth == 0 => return false,
            // A let-else scrutinee cannot end in `}`, so an `else` after one belongs to an `if`.
            b'e' if depth == 0
                && code[i..].starts_with("else")
                && bytes.get(i + 4).is_none_or(|b| !is_ident_byte(*b))
                && (i == 0 || !is_ident_byte(bytes[i - 1]))
                && !code[..i].trim_end().ends_with('}') =>
            {
                return true;
            }
            _ => {}
        }
        i += 1;
    }
    false
}

/// The `{` opening a match's arms: the first one outside the scrutinee's brackets.
fn match_arms_open(bytes: &[u8], from: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (i, b) in bytes.iter().enumerate().skip(from) {
        match b {
            b'{' if depth == 0 => return Some(i),
            b'(' | b'[' => depth += 1,
            b')' | b']' => depth = depth.checked_sub(1)?,
            b';' | b'}' if depth == 0 => return None,
            _ => {}
        }
    }
    None
}

/// (offset, pattern text) of each arm of the match whose arms open at `open`.
fn match_arm_patterns(code: &str, open: usize) -> Vec<(usize, &str)> {
    let bytes = code.as_bytes();
    let mut arms = Vec::new();
    let mut i = open + 1;
    loop {
        i = skip_whitespace(bytes, i);
        if i >= bytes.len() || bytes[i] == b'}' {
            return arms;
        }
        let start = i;
        let mut depth = 0usize;
        while i < bytes.len() {
            match bytes[i] {
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => {
                    if depth == 0 {
                        return arms;
                    }
                    depth -= 1;
                }
                b'=' if depth == 0 && bytes.get(i + 1) == Some(&b'>') => break,
                _ => {}
            }
            i += 1;
        }
        if i >= bytes.len() {
            return arms;
        }
        arms.push((start, &code[start..i]));

        i = skip_whitespace(bytes, i + 2);
        if bytes.get(i) == Some(&b'{') {
            i = skip_whitespace(bytes, balanced_end(bytes, i));
            if bytes.get(i) == Some(&b',') {
                i += 1;
            }
            continue;
        }
        let mut depth = 0usize;
        while i < bytes.len() {
            match bytes[i] {
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => {
                    if depth == 0 {
                        break;
                    }
                    depth -= 1;
                }
                b',' if depth == 0 => {
                    i += 1;
                    break;
                }
                _ => {}
            }
            i += 1;
        }
    }
}

/// `pattern` without a trailing `if` guard.
fn strip_guard(pattern: &str) -> &str {
    let bytes = pattern.as_bytes();
    let mut depth = 0usize;
    for i in 0..bytes.len() {
        match bytes[i] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth = depth.saturating_sub(1),
            b'i' if depth == 0
                && pattern[i..].starts_with("if")
                && bytes.get(i + 2).is_none_or(|b| !is_ident_byte(*b))
                && (i == 0 || !is_ident_byte(bytes[i - 1])) =>
            {
                return &pattern[..i];
            }
            _ => {}
        }
    }
    pattern
}

fn split_depth_zero(text: &str, separator: u8) -> Vec<&str> {
    let bytes = text.as_bytes();
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0;
    for i in 0..bytes.len() {
        match bytes[i] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth = depth.saturating_sub(1),
            b if b == separator && depth == 0 => {
                parts.push(&text[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&text[start..]);
    parts
}

/// `_`, or a pattern that binds whatever it is given: `other`, `ref mut x`, `x @ _`.
fn is_catch_all(alternative: &str) -> bool {
    let mut pattern = strip_reference(strip_attributes(alternative.trim()));
    if let Some((_, bound)) = pattern.split_once('@') {
        return is_catch_all(bound);
    }
    for modifier in ["ref ", "mut "] {
        if let Some(rest) = pattern.strip_prefix(modifier) {
            pattern = rest.trim_start();
        }
    }
    let mut chars = pattern.chars();
    match chars.next() {
        Some('_') if pattern.len() == 1 => true,
        Some(first) if first == '_' || first.is_ascii_lowercase() => {
            chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        }
        _ => false,
    }
}

/// 1-based lines where `source` builds a `std::process::Command`: `Command::new`, however
/// qualified, spaced or wrapped. Another type's `new` (`GitCommand::new`) is not matched.
pub fn constructs_process_command(source: &str) -> Vec<usize> {
    let code = code_without_strings(source);
    let bytes = code.as_bytes();
    let mut lines = BTreeSet::new();
    for offset in ident_offsets(&code, "Command") {
        let mut at = skip_whitespace(bytes, offset + "Command".len());
        if !code[at..].starts_with("::") {
            continue;
        }
        at = skip_whitespace(bytes, at + 2);
        if code[at..].starts_with("new") && bytes.get(at + 3).is_none_or(|b| !is_ident_byte(*b)) {
            lines.insert(line_at(&code, offset));
        }
    }
    lines.into_iter().collect()
}

/// Methods that put something into, or take something out of, a child process's environment.
const ENVIRONMENT_METHODS: &[&str] = &["env", "envs", "env_clear", "env_remove"];

/// 1-based lines where `source` calls one of [`ENVIRONMENT_METHODS`] as a method — `.env(`,
/// however wrapped or spaced. `env!(..)` and `std::env::var_os(..)` are not method calls and
/// are not matched; neither is a function merely named `env`.
pub fn configures_process_environment(source: &str) -> Vec<usize> {
    let code = code_without_strings(source);
    let bytes = code.as_bytes();
    let mut lines = BTreeSet::new();
    for name in ENVIRONMENT_METHODS {
        for offset in ident_offsets(&code, name) {
            let before = code[..offset].trim_end();
            let after = skip_whitespace(bytes, offset + name.len());
            if before.ends_with('.') && bytes.get(after) == Some(&b'(') {
                lines.insert(line_at(&code, offset));
            }
        }
    }
    lines.into_iter().collect()
}

/// Keywords a type name follows when it is being declared or implemented, not built.
const DECLARING_KEYWORDS: &[&str] = &["struct", "impl", "for", "enum", "union", "trait"];

/// The 1-based line of every literal in `source` that builds a value of the struct `name` —
/// `Name { .. }` or `Self { .. }`, including a struct update — one entry PER LITERAL, so two on
/// a line are two. Not matched: declaring it (`struct Name {`), implementing it (`impl Name {`,
/// `impl T for Name {`) or naming it as a return type (`-> Self {`). A destructuring pattern
/// `let Self { .. } = x` is read as a construction; the guard that uses this counts, so a false
/// positive there fails loudly rather than silently.
pub fn constructs_struct(source: &str, name: &str) -> Vec<usize> {
    struct_literals(source, &[name, "Self"])
}

/// [`constructs_struct`] without the `Self { .. }` spelling: for a file that does not implement
/// the type, where `Self` means something else. Pair it with [`implements_type`].
pub fn constructs_named_struct(source: &str, name: &str) -> Vec<usize> {
    struct_literals(source, &[name])
}

/// 1-based lines where `source` opens an `impl` block for the type `name` — `impl Name {` or
/// `impl T for Name {` — which is where a `Self { .. }` literal for it can be written.
pub fn implements_type(source: &str, name: &str) -> Vec<usize> {
    let code = code_without_strings(source);
    let mut lines = BTreeSet::new();
    for offset in ident_offsets(&code, name) {
        let before = code[..offset].trim_end();
        let implementing = ["impl", "for"].iter().any(|keyword| {
            before.ends_with(keyword)
                && before[..before.len() - keyword.len()]
                    .bytes()
                    .next_back()
                    .is_none_or(|b| !is_ident_byte(b))
        });
        if implementing {
            lines.insert(line_at(&code, offset));
        }
    }
    lines.into_iter().collect()
}

/// Whether `before` ends in `->`, allowing a reference, `mut` and a lifetime between it and the
/// type: `-> &'a mut Name {` is a function body opening, not a literal.
fn is_return_type_position(before: &str) -> bool {
    let mut head = before;
    loop {
        let trimmed = head.trim_end();
        let next = trimmed
            .strip_suffix('&')
            .or_else(|| trimmed.strip_suffix("mut"))
            .or_else(|| {
                let start = trimmed.rfind('\'')?;
                trimmed[start + 1..]
                    .bytes()
                    .all(is_ident_byte)
                    .then(|| &trimmed[..start])
            });
        match next {
            Some(shorter) if shorter.len() < head.len() => head = shorter,
            _ => return trimmed.ends_with("->"),
        }
    }
}

fn struct_literals(source: &str, idents: &[&str]) -> Vec<usize> {
    let code = code_without_strings(source);
    let bytes = code.as_bytes();
    let mut literals = Vec::new();
    for ident in idents {
        for offset in ident_offsets(&code, ident) {
            let after = skip_whitespace(bytes, offset + ident.len());
            if bytes.get(after) != Some(&b'{') {
                continue;
            }
            let before = code[..offset].trim_end();
            let declares = is_return_type_position(before)
                || DECLARING_KEYWORDS.iter().any(|keyword| {
                    before.ends_with(keyword)
                        && before[..before.len() - keyword.len()]
                            .bytes()
                            .next_back()
                            .is_none_or(|b| !is_ident_byte(b))
                });
            if !declares {
                literals.push(line_at(&code, offset));
            }
        }
    }
    literals.sort_unstable();
    literals
}

/// Keywords that open a type declaration.
const TYPE_KEYWORDS: &[&str] = &["struct", "enum"];

/// A `struct` or `enum` declared in a source: its name, the keyword, the 1-based line of the
/// keyword, and the text of its body (fields or variants).
#[derive(Debug, PartialEq, Eq)]
pub struct TypeDeclaration {
    pub name: String,
    pub keyword: &'static str,
    pub line: usize,
    pub body: String,
}

/// Every `struct` and `enum` declared in `source`, with the text between its braces (or
/// parentheses, for a tuple struct). A unit struct has an empty body.
pub fn type_declarations(source: &str) -> Vec<TypeDeclaration> {
    let code = code_without_strings(source);
    let bytes = code.as_bytes();
    let mut found = Vec::new();
    for keyword in TYPE_KEYWORDS {
        for offset in ident_offsets(&code, keyword) {
            // `struct` inside a `use` or as a field name would be odd Rust; a `struct` keyword is
            // followed by the type's name, which is what is read here.
            let name_start = skip_whitespace(bytes, offset + keyword.len());
            let name_end = (name_start..bytes.len())
                .find(|&i| !is_ident_byte(bytes[i]))
                .unwrap_or(bytes.len());
            if name_end == name_start {
                continue;
            }
            let name = code[name_start..name_end].to_owned();
            // Past generics and a where clause, to the body. A `(` after `where` is a bound
            // (`F: Fn(u8) -> u8`), not a tuple body; only a `{` can follow a where clause.
            let mut at = name_end;
            let mut depth = 0usize;
            let mut in_where = false;
            let body = loop {
                match bytes.get(at) {
                    None => break String::new(),
                    Some(b'<') => depth += 1,
                    Some(b'>') => depth = depth.saturating_sub(1),
                    Some(b';') if depth == 0 => break String::new(),
                    Some(b'{') if depth == 0 => {
                        let end = balanced_end(bytes, at);
                        break code[at + 1..end.saturating_sub(1).max(at + 1)].to_owned();
                    }
                    Some(b'(') if depth == 0 && !in_where => {
                        let end = balanced_end(bytes, at);
                        break code[at + 1..end.saturating_sub(1).max(at + 1)].to_owned();
                    }
                    Some(b'(') if depth == 0 => {
                        at = balanced_end(bytes, at);
                        continue;
                    }
                    Some(b'w')
                        if depth == 0
                            && code[at..].starts_with("where")
                            && bytes.get(at + 5).is_none_or(|b| !is_ident_byte(*b))
                            && !is_ident_byte(bytes[at - 1]) =>
                    {
                        in_where = true;
                    }
                    _ => {}
                }
                at += 1;
            };
            found.push(TypeDeclaration {
                name,
                keyword,
                line: line_at(&code, offset),
                body,
            });
        }
    }
    found.sort_by_key(|declaration| declaration.line);
    found
}

/// Names of every `struct` and `enum` in `source` whose body names any of `idents` — a type
/// that holds one of them in a field or a variant, directly.
pub fn types_containing(source: &str, idents: &[&str]) -> Vec<String> {
    type_declarations(source)
        .into_iter()
        .filter(|declaration| {
            idents
                .iter()
                .any(|ident| !ident_offsets(&declaration.body, ident).is_empty())
        })
        .map(|declaration| declaration.name)
        .collect()
}

/// 1-based lines of every `struct` (not `enum`) in `source` with a field naming one of `idents`.
pub fn structs_with_a_field_naming(source: &str, idents: &[&str]) -> Vec<usize> {
    type_declarations(source)
        .into_iter()
        .filter(|declaration| declaration.keyword == "struct")
        .filter(|declaration| {
            idents
                .iter()
                .any(|ident| !ident_offsets(&declaration.body, ident).is_empty())
        })
        .map(|declaration| declaration.line)
        .collect()
}

/// 1-based lines where `source` gives the type `name` one of `traits`: a `#[derive(..)]` on its
/// declaration naming the trait (however the path is spelled — `serde::Serialize` counts as
/// `Serialize`), or an `impl Trait for Name` block, generics and paths included.
pub fn derives_or_implements(source: &str, name: &str, traits: &[&str]) -> Vec<usize> {
    let code = code_without_strings(source);
    let bytes = code.as_bytes();
    let mut lines = BTreeSet::new();
    let last_segment = |path: &str| -> String {
        let path = path.trim();
        let without_generics = path.split('<').next().unwrap_or(path);
        without_generics
            .rsplit("::")
            .next()
            .unwrap_or(without_generics)
            .trim()
            .to_owned()
    };

    for offset in ident_offsets(&code, "derive") {
        let open = skip_whitespace(bytes, offset + "derive".len());
        if bytes.get(open) != Some(&b'(') || !code[..offset].trim_end().ends_with("#[") {
            continue;
        }
        let end = balanced_end(bytes, open);
        let derived: Vec<String> = code[open + 1..end.saturating_sub(1)]
            .split(',')
            .map(last_segment)
            .collect();
        if !derived.iter().any(|d| traits.contains(&d.as_str())) {
            continue;
        }
        // The declaration this attribute decorates: the next `struct`/`enum` keyword, past the
        // attribute's own `]` and any further attributes.
        let mut at = skip_whitespace(bytes, end);
        if bytes.get(at) == Some(&b']') {
            at += 1;
        }
        let declared = loop {
            at = skip_whitespace(bytes, at);
            if bytes.get(at) == Some(&b'#') {
                let attribute_open = skip_whitespace(bytes, at + 1);
                at = balanced_end(bytes, attribute_open);
                continue;
            }
            let word_end = (at..bytes.len())
                .find(|&i| !is_ident_byte(bytes[i]))
                .unwrap_or(bytes.len());
            let word = &code[at..word_end];
            if TYPE_KEYWORDS.contains(&word) {
                let name_start = skip_whitespace(bytes, word_end);
                let name_end = (name_start..bytes.len())
                    .find(|&i| !is_ident_byte(bytes[i]))
                    .unwrap_or(bytes.len());
                break Some(&code[name_start..name_end]);
            }
            if word.is_empty() || word_end == bytes.len() {
                break None;
            }
            // `pub`, `pub(crate)`, ...
            at = word_end;
            if bytes.get(skip_whitespace(bytes, at)) == Some(&b'(') {
                at = balanced_end(bytes, skip_whitespace(bytes, at));
            }
        };
        if declared == Some(name) {
            lines.insert(line_at(&code, offset));
        }
    }

    for offset in ident_offsets(&code, "impl") {
        let Some(open) = code[offset..].find('{') else {
            continue;
        };
        let header = &code[offset + "impl".len()..offset + open];
        // Generic parameters on the impl itself: `impl<T: Bound> ...`.
        let header = match header.trim_start().strip_prefix('<') {
            Some(_) => {
                let start = header.find('<').unwrap_or(0);
                let end = balanced_angle_end(header.as_bytes(), start);
                &header[end..]
            }
            None => header,
        };
        let Some((trait_path, for_type)) = header.split_once(" for ") else {
            continue;
        };
        let implemented = last_segment(trait_path);
        // The target's own name: past a reference, a lifetime and any path prefix
        // (`impl Debug for self::Held` names `Held`).
        let target = for_type.trim().trim_start_matches('&').trim_start();
        let target = match target.strip_prefix('\'') {
            Some(rest) => rest.trim_start_matches(is_ident_char).trim_start(),
            None => target,
        };
        let target_name = last_segment(target);
        let target_name = target_name
            .split(|c: char| !is_ident_char(c))
            .next()
            .unwrap_or("");
        if traits.contains(&implemented.as_str()) && target_name == name {
            lines.insert(line_at(&code, offset));
        }
    }
    lines.into_iter().collect()
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// 1-based lines where `source` gives the type `name` another name: `use path::Name as Other`
/// (in any `use` tree) or `type Other = path::Name;` / `type Other<..> = Name<..>;`. Past
/// either, code can hold the type without spelling its name, which is what a name-keyed guard
/// reads.
pub fn renames_type(source: &str, name: &str) -> Vec<usize> {
    let code = code_without_strings(source);
    let bytes = code.as_bytes();
    let mut lines = BTreeSet::new();
    for offset in ident_offsets(&code, name) {
        let after = skip_whitespace(bytes, offset + name.len());
        let aliased = code[after..].starts_with("as")
            && bytes.get(after + 2).is_some_and(u8::is_ascii_whitespace)
            && in_use_statement(&code, offset);
        if aliased {
            lines.insert(line_at(&code, offset));
            continue;
        }
        // `type X = ..Name..;` — the statement starts with `type` and has `=` before this name.
        let start = code[..offset].rfind(';').map_or(0, |at| at + 1);
        let statement = &code[start..offset];
        let is_type_item = ident_offsets(statement, "type").first().is_some_and(|at| {
            statement[..*at].trim().is_empty()
                || statement[..*at].trim_end().ends_with("pub")
                || statement[..*at].trim_end().ends_with(')')
        });
        if is_type_item && statement.contains('=') {
            lines.insert(line_at(&code, offset));
        }
    }
    lines.into_iter().collect()
}

/// One past the `>` closing the `<` at `open`.
fn balanced_angle_end(bytes: &[u8], open: usize) -> usize {
    let mut depth = 0usize;
    for (i, b) in bytes.iter().enumerate().skip(open) {
        match b {
            b'<' => depth += 1,
            b'>' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return i + 1;
                }
            }
            _ => {}
        }
    }
    bytes.len()
}

/// Macros that render their arguments: into a string, a stream, a panic message or a log
/// event. Bare names, so `tracing::info!` and `log::info!` match on `info`.
const RENDERING_MACROS: &[&str] = &[
    "format",
    "format_args",
    "log",
    "print",
    "println",
    "eprint",
    "eprintln",
    "write",
    "writeln",
    "panic",
    "assert",
    "assert_eq",
    "assert_ne",
    "debug_assert",
    "debug_assert_eq",
    "debug_assert_ne",
    "unreachable",
    "todo",
    "unimplemented",
    "dbg",
    "trace",
    "debug",
    "info",
    "warn",
    "error",
    "event",
    "span",
    "trace_span",
    "debug_span",
    "info_span",
    "warn_span",
    "error_span",
];

/// 1-based lines where `source` invokes one of [`RENDERING_MACROS`] with an argument list that
/// names any of `idents` — `format!("{:?}", holder)`, `tracing::info!(pw = s.expose_secret())`,
/// `assert_eq!(secret.expose_secret(), x)`, however wrapped.
pub fn renders_in_a_macro(source: &str, idents: &[&str]) -> Vec<usize> {
    let code = code_without_strings(source);
    let bytes = code.as_bytes();
    let mut lines = BTreeSet::new();
    for name in RENDERING_MACROS {
        for offset in ident_offsets(&code, name) {
            let bang = skip_whitespace(bytes, offset + name.len());
            if bytes.get(bang) != Some(&b'!') {
                continue;
            }
            let open = skip_whitespace(bytes, bang + 1);
            if !matches!(bytes.get(open), Some(b'(' | b'[' | b'{')) {
                continue;
            }
            let end = balanced_end(bytes, open);
            let arguments = &code[open..end];
            if idents
                .iter()
                .any(|ident| !ident_offsets(arguments, ident).is_empty())
            {
                lines.insert(line_at(&code, offset));
            }
        }
    }
    lines.into_iter().collect()
}

/// 1-based lines where `source` spawns a `git` subprocess, matched on the literal program name.
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

    /// Spelled out, not looped over the roster: a loop would shrink with the roster.
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
        // Prose inside a string literal is still prose.
        assert!(waits_on_work("status.set(\"waiting to receive a lock\");").is_empty());
        assert!(waits_on_work("let hint = r#\"sleep until the Mutex frees\"#;").is_empty());
    }

    /// Caught by: a quote inside a char literal blanking the rest of the file.
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

    /// Caught by: blanking from one lifetime's apostrophe to the next.
    #[test]
    fn a_lifetime_is_not_mistaken_for_a_char_literal() {
        let src = "fn f<'a, 'b>(x: &'a str, y: &'b Mutex) { let _ = x.recv(); }";
        assert_eq!(
            waits_on_work(src),
            vec![1],
            "a lifetime pair blanked the code between them"
        );
        // The escaped-apostrophe literal closes on the third apostrophe.
        let src = "let tick = '\\''; let _ = rx.recv();";
        assert_eq!(waits_on_work(src), vec![1]);
    }

    /// Caught by: `code_only` opening a char literal on a lifetime's apostrophe, then closing
    /// it on an apostrophe inside a later string, after which `//` in that string reads as a
    /// comment and the rest of the file is blanked.
    #[test]
    fn a_lifetime_before_a_string_with_an_apostrophe_and_a_slash_pair_hides_nothing_after() {
        let src = "fn f(x: &'static str) {}\nlet t = \"Password for 'https://h/x': \";\nlet _ = rx.recv();\n";
        assert_eq!(
            waits_on_work(src),
            vec![3],
            "the code after the string was blanked"
        );
        assert!(
            code_without_strings(src).contains("recv"),
            "{:?}",
            code_without_strings(src)
        );
    }

    /// Caught by: replacing a `\\`+newline continuation with two spaces, which loses a line.
    #[test]
    fn a_string_continued_over_a_line_keeps_the_line_count() {
        let src = "let s = \"one \\\n    two\";\nlet _ = rx.recv();\n";
        assert_eq!(
            code_without_strings(src).lines().count(),
            src.lines().count()
        );
        assert_eq!(waits_on_work(src), vec![3]);
    }

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

        // The same token in production is still seen.
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

    fn parsed(manifest: &str) -> toml::Table {
        manifest.parse().unwrap()
    }

    fn names(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    #[test]
    fn every_dependency_table_is_read_and_test_only_ones_are_told_apart() {
        let declared = declared_dependencies(&parsed(
            "[package]\nname = \"x\"\n\
             [dependencies]\nfreya = \"1\"\n\
             [build-dependencies]\ncc = \"1\"\n\
             [dev-dependencies]\ngix = \"1\"\n\
             [target.'cfg(unix)'.dependencies]\nlibc = \"1\"\n\
             [target.'cfg(unix)'.dev-dependencies]\ncairn-git = { path = \"../cairn-git\" }\n",
        ));
        assert_eq!(declared.shipped, names(&["cc", "freya", "libc"]));
        assert_eq!(declared.test_only, names(&["cairn-git", "gix"]));
    }

    #[test]
    fn a_renamed_dependency_is_declared_under_its_package_name() {
        let declared = declared_dependencies(&parsed(
            "[dev-dependencies]\nbackend = { package = \"gix\", version = \"1\" }\n\
             [dependencies]\nengine = { workspace = true, package = \"cairn-git\" }\n",
        ));
        assert_eq!(declared.test_only, names(&["gix"]));
        assert_eq!(declared.shipped, names(&["cairn-git"]));
    }

    #[test]
    fn a_manifest_with_no_dependencies_declares_none() {
        assert_eq!(
            declared_dependencies(&parsed("[package]\nname = \"x\"\n")),
            DeclaredDependencies::default()
        );
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
