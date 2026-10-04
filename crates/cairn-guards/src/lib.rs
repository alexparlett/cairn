//! Matchers and repository walking for the invariant guards.

use std::collections::{BTreeMap, BTreeSet};
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

/// Every string literal of `source` outside comments and `#[cfg(test)]` modules, as (1-based
/// line it opens on, its text between the quotes as written, escapes not interpreted).
/// Plain, byte and raw strings (`"..."`, `b"..."`, `r#"..."#`) all count; a char literal
/// (`'"'`) does not open one.
pub fn production_string_literals(source: &str) -> Vec<(usize, String)> {
    let code = code_only(source);
    let strings_blanked = code_without_strings(source);
    let tests_blanked = code_without_test_modules(&strings_blanked);
    // A line a test module blanked: it had code, and has none left.
    let in_test: Vec<bool> = strings_blanked
        .lines()
        .zip(tests_blanked.lines())
        .map(|(before, after)| !before.trim().is_empty() && after.trim().is_empty())
        .collect();
    let bytes = code.as_bytes();
    let is_ident = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_';
    let mut found = Vec::new();
    let mut line = 1usize;
    let mut i = 0usize;
    while i < bytes.len() {
        let byte = bytes[i];
        let after_ident = i > 0 && is_ident(bytes[i - 1]);
        // `r"`, `r#"`, `br"`: a raw string, ended by a quote and as many hashes.
        let raw_hashes = (byte == b'r'
            && (!after_ident || (bytes[i - 1] == b'b' && (i < 2 || !is_ident(bytes[i - 2])))))
        .then(|| bytes[i + 1..].iter().take_while(|&&c| c == b'#').count())
        .filter(|hashes| bytes.get(i + 1 + hashes) == Some(&b'"'));
        if let Some(hashes) = raw_hashes {
            let start = i + hashes + 2;
            let mut end = start;
            while end < bytes.len()
                && !(bytes[end] == b'"'
                    && bytes[end + 1..].iter().take_while(|&&c| c == b'#').count() >= hashes)
            {
                end += 1;
            }
            let text = &code[start..end.min(code.len())];
            found.push((line, text.to_owned()));
            line += text.matches('\n').count();
            i = end + hashes + 1;
            continue;
        }
        match byte {
            b'\n' => {
                line += 1;
                i += 1;
            }
            b'\'' => match char_literal_end(bytes, i) {
                Some(end) => i = end + 1,
                None => i += 1,
            },
            b'"' => {
                let start = i + 1;
                let mut end = start;
                while end < bytes.len() && bytes[end] != b'"' {
                    end += if bytes[end] == b'\\' { 2 } else { 1 };
                }
                let end = end.min(bytes.len());
                let text = &code[start..end];
                found.push((line, text.to_owned()));
                line += text.matches('\n').count();
                i = end + 1;
            }
            _ => i += 1,
        }
    }
    found
        .into_iter()
        .filter(|(line, _)| !in_test.get(line - 1).copied().unwrap_or(false))
        .collect()
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

/// Every spelling of a keyboard modifier the toolkit offers a render file (D5, PRD R8.3),
/// read from the vendored `keyboard-types` (the version `Cargo.lock` pins) and Freya's
/// `ModifiersExt`: the held-keys type and its helper trait, the event field that carries
/// them, the type's constants (`Modifiers::CONTROL`), the modifier keys themselves
/// (`NamedKey::Control`, `Code::ControlLeft`), the lock keys (`CAPS_LOCK`, `NamedKey::NumLock`,
/// `Code::ScrollLock`: a component reading one is reading the held keys) and the platform
/// helpers that pick one. Whole identifiers, so an alias is caught on its import line and a
/// qualified path on its last segment. `Fn`, the function key, is matched only after `::`:
/// bare, it is Rust's closure trait.
pub const MODIFIER_IDENTS: &[&str] = &[
    // The type, its OS-aware helper trait and what it offers, and the event's field.
    "Modifiers",
    "ModifiersExt",
    "ctrl_or_meta",
    "ctrl_or_alt",
    "modifiers",
    // `keyboard_types::Modifiers`' constants.
    "ALT",
    "ALT_GRAPH",
    "CONTROL",
    "FN",
    "FN_LOCK",
    "HYPER",
    "META",
    "SHIFT",
    "SUPER",
    "SYMBOL",
    "SYMBOL_LOCK",
    "CAPS_LOCK",
    "NUM_LOCK",
    "SCROLL_LOCK",
    // `NamedKey`'s modifier keys.
    "Alt",
    "AltGraph",
    "Control",
    "FnLock",
    "Hyper",
    "Meta",
    "Shift",
    "Super",
    "Symbol",
    "SymbolLock",
    // The lock keys, `NamedKey`'s and `Code`'s alike.
    "CapsLock",
    "NumLock",
    "ScrollLock",
    // `Code`'s.
    "AltLeft",
    "AltRight",
    "ControlLeft",
    "ControlRight",
    "MetaLeft",
    "MetaRight",
    "ShiftLeft",
    "ShiftRight",
];

/// `Modifiers`' own predicates, read as nullary method calls (`.ctrl()`): a value of the type
/// reached by inference names nothing else.
pub const MODIFIER_METHODS: &[&str] = &["alt", "ctrl", "meta", "shift"];

/// A modifier written for a person to read: a key name in a label or a tooltip, which is a
/// chord a component spelled for one platform — joined by `+` or by `-`, as both are
/// written (`Shift+click`, `Shift-click`), and in a string or a char literal alike.
pub const MODIFIER_TEXT: &[&str] = &[
    "Ctrl", "Cmd", "⌘", "⌥", "⌃", "⇧", "Alt+", "Control+", "Meta+", "Option+", "Opt+", "Shift+",
    "Super+", "Command+", "Alt-", "Control-", "Meta-", "Option-", "Shift-", "Super-", "Ctrl-",
    "Cmd-",
];

/// 1-based lines where the production code of `source` (test modules blanked) names a
/// keyboard modifier: an identifier of [`MODIFIER_IDENTS`], `::Fn`, a nullary call of one of
/// [`MODIFIER_METHODS`], or a literal spelling a chord ([`spells_a_chord`]).
pub fn names_a_literal_modifier(source: &str) -> Vec<usize> {
    let code = code_without_test_modules(&code_without_strings(source));
    let mut lines = BTreeSet::new();
    for ident in MODIFIER_IDENTS {
        for offset in ident_offsets(&code, ident) {
            lines.insert(line_at(&code, offset));
        }
    }
    for offset in ident_offsets(&code, "Fn") {
        if code[..offset].trim_end().ends_with("::") {
            lines.insert(line_at(&code, offset));
        }
    }
    for name in MODIFIER_METHODS {
        for offset in ident_offsets(&code, name) {
            if code[..offset].trim_end().ends_with('.')
                && takes_no_arguments(&code, offset + name.len())
            {
                lines.insert(line_at(&code, offset));
            }
        }
    }
    lines.extend(spells_a_chord(source));
    lines.into_iter().collect()
}

/// 1-based lines where a production string or char literal of `source` holds one of
/// [`MODIFIER_TEXT`] once its escapes are read — `'⌘'`, `"\u{2318}1"` and `"\x41lt+1"`
/// spell a chord as surely as `"⌘1"` does.
pub fn spells_a_chord(source: &str) -> Vec<usize> {
    let mut lines = BTreeSet::new();
    let literals = production_string_literals(source)
        .into_iter()
        // A string's bytes arrive one `char` each (`code_only`); put them back together.
        .map(|(line, text)| (line, narrowed(&text)))
        .chain(production_char_literals(source));
    for (line, text) in literals {
        let text = unescaped(&text);
        if MODIFIER_TEXT.iter().any(|spelling| text.contains(spelling)) {
            lines.insert(line);
        }
    }
    lines.into_iter().collect()
}

/// `text` whose every `char` is one byte widened, as `code_only` copies a string, read back
/// as the UTF-8 it was.
fn narrowed(text: &str) -> String {
    let bytes: Vec<u8> = text
        .chars()
        .map(|c| u8::try_from(u32::from(c)).unwrap_or(b'?'))
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// A literal's text with its escapes read: `\u{..}`, `\x..`, the one-letter escapes, and a
/// line continuation. An escape that does not parse is kept as written.
fn unescaped(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('u') if chars.peek() == Some(&'{') => {
                chars.next();
                let digits: String = chars.by_ref().take_while(|c| *c != '}').collect();
                match u32::from_str_radix(&digits.replace('_', ""), 16)
                    .ok()
                    .and_then(char::from_u32)
                {
                    Some(decoded) => out.push(decoded),
                    None => out.push_str(&format!("\\u{{{digits}}}")),
                }
            }
            Some('x') => {
                let digits: String = chars.by_ref().take(2).collect();
                match u8::from_str_radix(&digits, 16) {
                    Ok(byte) => out.push(char::from(byte)),
                    Err(_) => out.push_str(&format!("\\x{digits}")),
                }
            }
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('0') => out.push('\0'),
            // A line continuation: the newline and the indentation after it are not text.
            Some('\n') => {
                while chars.peek().is_some_and(|c| c.is_whitespace()) {
                    chars.next();
                }
            }
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

/// Every char literal of `source` outside comments and `#[cfg(test)]` modules, as (1-based
/// line, its text between the apostrophes as written, escapes not interpreted). A lifetime
/// is not one, and neither is an apostrophe inside a string.
pub fn production_char_literals(source: &str) -> Vec<(usize, String)> {
    let code = code_only(source);
    let strings_blanked = code_without_strings(source);
    let tests_blanked = code_without_test_modules(&strings_blanked);
    // A line a test module blanked: it had code, and has none left.
    let in_test: Vec<bool> = strings_blanked
        .lines()
        .zip(tests_blanked.lines())
        .map(|(before, after)| !before.trim().is_empty() && after.trim().is_empty())
        .collect();
    let bytes = code.as_bytes();
    let mut found = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            // Strings are stepped over whole, raw ones by their closing quote and hashes.
            b'r' if bytes.get(i + 1).is_some_and(|b| *b == b'"' || *b == b'#')
                && (i == 0
                    || !is_ident_byte(bytes[i - 1])
                    || (bytes[i - 1] == b'b' && (i < 2 || !is_ident_byte(bytes[i - 2])))) =>
            {
                let hashes = bytes[i + 1..].iter().take_while(|&&c| c == b'#').count();
                if bytes.get(i + 1 + hashes) != Some(&b'"') {
                    i += 1;
                    continue;
                }
                let mut end = i + hashes + 2;
                while end < bytes.len()
                    && !(bytes[end] == b'"'
                        && bytes[end + 1..].iter().take_while(|&&c| c == b'#').count() >= hashes)
                {
                    end += 1;
                }
                i = end + hashes + 1;
            }
            b'"' => {
                let mut end = i + 1;
                while end < bytes.len() && bytes[end] != b'"' {
                    end += if bytes[end] == b'\\' { 2 } else { 1 };
                }
                i = end + 1;
            }
            b'\'' => match char_literal_end(bytes, i) {
                Some(end) => {
                    let line = line_at(&code, i);
                    if !in_test.get(line - 1).copied().unwrap_or(false) {
                        found.push((line, code[i + 1..end].to_owned()));
                    }
                    i = end + 1;
                }
                None => i += 1,
            },
            _ => i += 1,
        }
    }
    found
}

/// What a render file names when it builds an element: Freya's element constructors and
/// the types and traits a component is made of. The accelerator table holds data and the
/// resolution of a press against it, so naming one of these there is a view in the table.
pub const ELEMENT_BUILDERS: &[&str] = &[
    "rect",
    "label",
    "paragraph",
    "svg",
    "image",
    "Element",
    "IntoElement",
    "Component",
    "Button",
    "Label",
    "Rect",
];

/// 1-based lines where the production code of `source` (test modules blanked) names one of
/// [`ELEMENT_BUILDERS`].
pub fn names_an_element(source: &str) -> Vec<usize> {
    let code = code_without_test_modules(&code_without_strings(source));
    let mut lines = BTreeSet::new();
    for ident in ELEMENT_BUILDERS {
        for offset in ident_offsets(&code, ident) {
            lines.insert(line_at(&code, offset));
        }
    }
    lines.into_iter().collect()
}

const ROW_CONTENT: &str = "RowContent";

/// 1-based lines where `source` reads a `RowContent` without naming every variant: a wildcard or
/// catch-all arm in a match that names it (including `Some(_)` beside `Some(RowContent::..)`),
/// `if let`/`while let`/let-chain/`let .. else` over it, `matches!` over it, or an import of
/// its variants or of it under another name.
pub fn reads_row_content_partially(source: &str) -> Vec<usize> {
    reads_enum_partially(source, ROW_CONTENT)
}

/// [`reads_row_content_partially`] for any enum a view must read by naming every variant —
/// `RowContent`, and `DiffContent`, whose every state is something a view draws (R6.8).
pub fn reads_enum_partially(source: &str, name: &str) -> Vec<usize> {
    let code = code_without_strings(source);
    let bytes = code.as_bytes();
    let names_the_enum = |text: &str| !ident_offsets(text, name).is_empty();
    let mut lines = BTreeSet::new();

    for offset in ident_offsets(&code, name) {
        let rest = code[offset + name.len()..].trim_start();
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
        if names_the_enum(&code[at..end]) {
            lines.insert(line_at(&code, offset));
        }
    }

    for offset in ident_offsets(&code, "let") {
        let Some(assign) = depth_zero_assignment(bytes, offset + "let".len()) else {
            continue;
        };
        if !names_the_enum(&code[offset..assign]) {
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
        if !arms.iter().any(|(_, pattern)| names_the_enum(pattern)) {
            continue;
        }
        let wrappers: BTreeSet<&str> = arms
            .iter()
            .flat_map(|(_, pattern)| split_depth_zero(strip_guard(pattern), b'|'))
            .filter_map(|alternative| wrapped(alternative))
            .filter(|(_, inner)| names_the_enum(inner))
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
    calls_associated_function(source, "Command", "new")
}

/// 1-based lines where `source` names the associated function `function` of the type `name` —
/// `Name::function`, however qualified, spaced or wrapped, called or passed as a value. Another
/// type's function of the same name (`GitCommand::new` for `Command`) is not matched, and neither
/// is a longer function name (`Name::new_with`).
pub fn calls_associated_function(source: &str, name: &str, function: &str) -> Vec<usize> {
    let code = code_without_strings(source);
    let bytes = code.as_bytes();
    let mut lines = BTreeSet::new();
    for offset in ident_offsets(&code, name) {
        let mut at = skip_whitespace(bytes, offset + name.len());
        if !code[at..].starts_with("::") {
            continue;
        }
        at = skip_whitespace(bytes, at + 2);
        if code[at..].starts_with(function)
            && bytes
                .get(at + function.len())
                .is_none_or(|b| !is_ident_byte(*b))
        {
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
    calls_method(source, ENVIRONMENT_METHODS)
}

/// 1-based lines where `source` calls a method named in `names` — `.name(`, with or without
/// arguments, however spaced or wrapped. A free function or a definition of the same name is
/// not a method call and is not matched, and neither is a longer identifier.
pub fn calls_method(source: &str, names: &[&str]) -> Vec<usize> {
    method_calls(source, names, false)
}

/// [`calls_method`] for calls with no arguments only — `.name()`, however spaced or wrapped.
/// For a method with an innocent namesake that takes arguments: `Child::wait()` beside
/// `Condvar::wait(guard)`, `Command::spawn()` beside `thread::Builder::spawn(f)`.
pub fn calls_nullary_method(source: &str, names: &[&str]) -> Vec<usize> {
    method_calls(source, names, true)
}

fn method_calls(source: &str, names: &[&str], nullary_only: bool) -> Vec<usize> {
    let code = code_without_strings(source);
    let bytes = code.as_bytes();
    let mut lines = BTreeSet::new();
    for name in names {
        for offset in ident_offsets(&code, name) {
            let before = code[..offset].trim_end();
            let after = offset + name.len();
            let called = if nullary_only {
                takes_no_arguments(&code, after)
            } else {
                bytes.get(skip_whitespace(bytes, after)) == Some(&b'(')
            };
            if before.ends_with('.') && called {
                lines.insert(line_at(&code, offset));
            }
        }
    }
    lines.into_iter().collect()
}

/// 1-based lines where `source` declares `name` with unrestricted visibility — `pub struct
/// Name`, `pub fn name`, `pub enum`, `pub trait`, `pub type`, `pub const`, `pub static`,
/// `pub mod` — or re-exports it with a bare `pub use`. A restricted visibility (`pub(crate)`,
/// `pub(super)`, `pub(in ..)`) is not matched: it does not reach another crate.
pub fn declares_publicly(source: &str, name: &str) -> Vec<usize> {
    const ITEM_KEYWORDS: &[&str] = &[
        "struct", "fn", "enum", "trait", "type", "const", "static", "mod", "union",
    ];
    let code = code_without_strings(source);
    let mut lines = BTreeSet::new();
    for offset in ident_offsets(&code, name) {
        let before = code[..offset].trim_end();
        let declared = ITEM_KEYWORDS.iter().any(|keyword| {
            before.ends_with(keyword)
                && before[..before.len() - keyword.len()]
                    .bytes()
                    .next_back()
                    .is_none_or(|b| !is_ident_byte(b))
                && unrestricted_pub_before(&before[..before.len() - keyword.len()])
        });
        if declared {
            lines.insert(line_at(&code, offset));
        }
    }
    // A `use` statement runs to its `;` and holds none inside, however its groups nest.
    for start in ident_offsets(&code, "use") {
        let end = code[start..]
            .find(';')
            .map_or(code.len(), |end| start + end);
        let Some(&at) = ident_offsets(&code[start..end], name).first() else {
            continue;
        };
        if unrestricted_pub_before(&code[..start]) {
            lines.insert(line_at(&code, start + at));
        }
    }
    lines.into_iter().collect()
}

/// Whether `before` — the text ahead of an item keyword — ends in a bare `pub`, allowing the
/// qualifiers that may sit between them (`pub const fn`, `pub unsafe fn`, `pub async fn`).
fn unrestricted_pub_before(before: &str) -> bool {
    let mut head = before.trim_end();
    for qualifier in ["const", "async", "unsafe", "extern"] {
        if let Some(rest) = head.strip_suffix(qualifier)
            && rest.bytes().next_back().is_none_or(|b| !is_ident_byte(b))
        {
            head = rest.trim_end();
        }
    }
    head.strip_suffix("pub")
        .is_some_and(|rest| rest.bytes().next_back().is_none_or(|b| !is_ident_byte(b)))
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

/// gitoxide's mutation API, enumerated from the vendored gix 0.87.1 source (and the sub-crates
/// its lockfile pins: gix-ref 0.67.1, gix-index 0.55.0, gix-object 0.64.1, gix-odb 0.84.0,
/// gix-lock 24.0.0, gix-tempfile 24.0.0, gix-fs 0.22.1, gix-worktree 0.56.0, gix-worktree-state
/// 0.34.1, gix-pack 0.74.2, gix-merge 0.20.1, gix-note 0.1.1), never from memory. Each entry is
/// something that writes to a repository on disk — refs and reflogs, objects, the index, the
/// working tree, lock and temporary files, a new or cloned repository — or the one entry point
/// to such a write. Paths cite `gix-0.87.1/src/` unless another crate is named.
///
/// Identifiers matched wherever they appear, because nothing that reads has their name:
pub const GITOXIDE_MUTATION_IDENTS: &[&str] = &[
    // Objects. repository/object.rs: `write_object` :250, `write_blob` :289,
    // `write_blob_stream` :308, `commit_as` :361, `new_commit` :483, `new_commit_as` :499.
    "write_object",
    "write_blob",
    "write_blob_stream",
    "commit_as",
    "new_commit",
    "new_commit_as",
    // The object-database `Write` trait, implemented for `Repository` (repository/impls.rs:109)
    // and every store that hits disk (gix-object traits/mod.rs:15, :22, :40).
    "write_buf",
    "write_buf_with_known_id",
    "write_stream_with_known_id",
    // A blob written from a working-tree file (filter.rs:220).
    "worktree_file_to_object",
    // Merges write the merged blobs, and the virtual base's trees and commits
    // (repository/merge.rs:126, :181, :241, :253).
    "merge_trees",
    "merge_commits",
    "virtual_merge_base",
    "virtual_merge_base_with_graph",
    // Tree editing: the editor and its in-memory edits are the entry to `Editor::write`
    // (object/tree/editor.rs:185), whose name `.write(` is matched below (repository/object.rs:22;
    // editor.rs:162, :179).
    "edit_tree",
    "upsert",
    "remove_leaf",
    // Refs and reflogs (repository/reference.rs:15, :136, :147, :157; reference/edits.rs:30;
    // repository/branch.rs:77, which also rewrites `.git/config` under a lock).
    "tag_reference",
    "edit_reference",
    "edit_references",
    "edit_references_as",
    "set_target_id",
    "delete_local_branches",
    // The index, written back after a status (status/iter/types.rs:87).
    "write_changes",
    // Notes (note.rs:180).
    "replace_at_ref",
    // Clone and fetch: checkout of a clone (clone/checkout.rs:73), and the network paths, off in
    // Cairn's feature set today but named so turning them on is no hole (clone/fetch/mod.rs:106,
    // :427; remote/connection/fetch/mod.rs:141).
    "main_worktree",
    "fetch_only",
    "fetch_then_checkout",
    "prepare_fetch",
    "prepare_clone",
    "prepare_clone_bare",
    "PrepareFetch",
    "PrepareCheckout",
    // A new repository (lib.rs:341; init.rs:65).
    "init_bare",
    "init_opts",
    // Packs (gix-pack multi_index/write.rs:87; bundle/write/mod.rs:63, :175;
    // index/write/mod.rs:99).
    "write_from_index_paths",
    "write_to_directory",
    "write_to_directory_eagerly",
    "write_data_iter_to_stream",
    // Lock and temporary files (gix-lock acquire.rs:73, :92, :122, :142; gix-tempfile
    // lib.rs:222, :231, :241, :250).
    "acquire_to_update_resource",
    "acquire_to_update_resource_with_permissions",
    "acquire_to_hold_resource",
    "acquire_to_hold_resource_with_permissions",
    "writable_at",
    "writable_at_with_permissions",
    "mark_at",
    "mark_at_with_permissions",
    // A working-tree stack that creates directories and unlinks what is in the way
    // (gix-worktree stack/state/mod.rs:63), and probe files written into a directory
    // (gix-fs capabilities.rs:63).
    "for_checkout",
    "probe_dir",
];

/// Names that write only when called — `.name(` or `::name(` — because as a path segment or a
/// plain identifier they are also read vocabulary (`gix::traverse::commit::..`, `gix::tag`,
/// `gix::reference::iter`, a local named `reference`):
pub const GITOXIDE_MUTATION_CALLS: &[&str] = &[
    // repository/object.rs:461 and :337; repository/reference.rs:79; also gix-ref's
    // `Transaction::commit` (store/file/transaction/commit.rs:28) and gix-lock's `commit`.
    "commit",
    "tag",
    "reference",
    // reference/edits.rs:61.
    "delete",
    // Tree editing from a `Tree` (object/tree/editor.rs:81).
    "edit",
    // gix-ref store/file/transaction/mod.rs:70, prepare.rs:232, :439.
    "transaction",
    "prepare",
    "rollback",
    // Fetch's last step, which writes the pack and the refs (receive_pack.rs:73).
    "receive",
    // A signed commit, written (object/commit.rs:240).
    "signed",
    // A clone kept on disk (clone/access.rs:105, clone/checkout.rs:166).
    "persist",
    // The one entry to notes, whose `replace` and `remove` (note.rs:159, :217) share their names
    // with std's collections and strings; Cairn reads no notes, so a note read would be built in
    // `ops/` beside the writes.
    "notes",
];

/// Names that write when called as a METHOD — `.name(` — and are something else as a path call:
/// `std::fs::write(..)`, `gix::worktree::archive::write_stream(..)`. The index file's `write`
/// (gix-index file/write.rs:67), a tree editor's (object/tree/editor.rs:185, :274) and the
/// object-database `Write` trait's (gix-object traits/mod.rs:9, :30); `write_to` serialises onto
/// any writer, the index's and the ref log's among them. In `cairn-git`, which does not link the
/// toolkit, nothing else called `.write(` is in use outside `ops/`.
pub const GITOXIDE_MUTATION_METHODS: &[&str] = &["write", "write_to", "write_stream"];

/// Paths whose naming means a write: modules and re-exports that hold only write vocabulary
/// (lib.rs:141 `lock`, :155 `tempfile`; gix-ref store/file/transaction; worktree/mod.rs:8
/// `state`, the checkout; merge.rs:1 and note.rs:6 `plumbing`; gix-fs `symlink` and `dir`), and
/// the `Write` trait's re-exports (prelude.rs:2), so a fully qualified trait call is seen too.
pub const GITOXIDE_MUTATION_PATHS: &[&[&str]] = &[
    &["gix", "lock"],
    &["gix", "tempfile"],
    &["refs", "transaction"],
    &["worktree", "state"],
    &["merge", "plumbing"],
    &["note", "plumbing"],
    &["gix", "fs", "symlink"],
    &["gix", "fs", "dir"],
    &["gix", "prelude"],
    &["objs", "Write"],
];

/// Paths that write when they end there — `gix::init(..)`, `use gix::init;` — and are a read
/// namespace when the path goes on (`gix::init::Error`): lib.rs:332, init.rs:49, create.rs:170,
/// gix-fs capabilities.rs:53.
pub const GITOXIDE_MUTATION_PATH_ENDS: &[&[&str]] = &[
    &["gix", "init"],
    &["ThreadSafeRepository", "init"],
    &["create", "into"],
    &["Capabilities", "probe"],
];

/// Every gitoxide mutation API `source` names, as `line: what`, over its code with comments
/// and strings blanked. The rosters above are the whole definition.
pub fn names_gitoxide_mutation(source: &str) -> Vec<String> {
    let code = code_without_strings(source);
    let bytes = code.as_bytes();
    let mut found = BTreeSet::new();
    for ident in GITOXIDE_MUTATION_IDENTS {
        for offset in ident_offsets(&code, ident) {
            found.insert((line_at(&code, offset), (*ident).to_owned()));
        }
    }
    for name in GITOXIDE_MUTATION_CALLS {
        for offset in ident_offsets(&code, name) {
            let before = code[..offset].trim_end();
            let called = bytes.get(skip_whitespace(bytes, offset + name.len())) == Some(&b'(');
            if called && (before.ends_with('.') || before.ends_with("::")) {
                found.insert((line_at(&code, offset), format!("{name}(..)")));
            }
        }
    }
    for line in calls_method(&code, GITOXIDE_MUTATION_METHODS) {
        found.insert((line, ".write*(..)".to_owned()));
    }
    for path in GITOXIDE_MUTATION_PATHS {
        for (start, _) in path_offsets(&code, path) {
            found.insert((line_at(&code, start), path.join("::")));
        }
    }
    for path in GITOXIDE_MUTATION_PATH_ENDS {
        for (start, end) in path_offsets(&code, path) {
            if !code[skip_whitespace(bytes, end)..].starts_with("::") {
                found.insert((line_at(&code, start), path.join("::")));
            }
        }
    }
    found
        .into_iter()
        .map(|(line, what)| format!("{line}: {what}"))
        .collect()
}

/// Where `segments` appear in `code` as one path — `a::b::c`, whitespace allowed around each
/// `::` — as (start, end) byte offsets.
fn path_offsets(code: &str, segments: &[&str]) -> Vec<(usize, usize)> {
    let bytes = code.as_bytes();
    let Some((first, rest)) = segments.split_first() else {
        return Vec::new();
    };
    let mut found = Vec::new();
    'start: for start in ident_offsets(code, first) {
        let mut at = start + first.len();
        for segment in rest {
            let separator = skip_whitespace(bytes, at);
            if !code[separator..].starts_with("::") {
                continue 'start;
            }
            let next = skip_whitespace(bytes, separator + 2);
            let end = next + segment.len();
            if !code[next..].starts_with(segment)
                || bytes.get(end).is_some_and(|b| is_ident_byte(*b))
            {
                continue 'start;
            }
            at = end;
        }
        found.push((start, at));
    }
    found
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

/// The entries of one job's own `env:` block in a GitHub Actions workflow, trimmed — the
/// variables every step of that job sees. A job is a two-space-indented key (`  gate:`)
/// inside the top-level `jobs:` block — never a key of the same spelling under `on:` or
/// anywhere else — running to the next key at two spaces or less; its `env:` is the one
/// indented four spaces directly under it, so a workflow-level `env:`, another job's, and a
/// step's own (`        env:`, which only that step sees) are none of them. An entry is a
/// line at the env block's own indent (its first entry's); a line deeper than that is a
/// value's continuation — the body of a block scalar (`X: |`) or a nested map — and is
/// none, nor is the body of a block scalar anywhere in the job read as a key. Comments
/// and blank lines are not entries.
pub fn job_env_entries(workflow: &str, job: &str) -> Vec<String> {
    let indent = |line: &str| line.len() - line.trim_start().len();
    let meaningful = |line: &str| {
        let trimmed = line.trim();
        !trimmed.is_empty() && !trimmed.starts_with('#')
    };
    // A key whose value is a block scalar (`|`, `>`, with a chomping or indentation
    // indicator, and a comment after it), whose body is every deeper line after it.
    let opens_block_scalar = |line: &str| {
        let value = line
            .split_once(": ")
            .map(|(_, value)| value)
            .or_else(|| line.trim_start().strip_prefix("- "))
            .unwrap_or_default();
        let value = value.split(" #").next().unwrap_or_default().trim();
        value.starts_with(['|', '>'])
            && value[1..]
                .chars()
                .all(|c| c == '-' || c == '+' || c.is_ascii_digit())
    };
    let lines: Vec<&str> = workflow.lines().collect();
    let Some(jobs) = lines.iter().position(|line| line.trim_end() == "jobs:") else {
        return Vec::new();
    };
    let jobs_block: Vec<&str> = lines[jobs + 1..]
        .iter()
        .take_while(|line| !(meaningful(line) && indent(line) == 0))
        .copied()
        .collect();
    let header = format!("  {job}:");
    let mut scalar: Option<usize> = None;
    let mut at = None;
    for (index, line) in jobs_block.iter().enumerate() {
        if let Some(opened) = scalar {
            if !meaningful(line) || indent(line) > opened {
                continue;
            }
            scalar = None;
        }
        if line.trim_end() == header {
            at = Some(index);
            break;
        }
        if meaningful(line) && opens_block_scalar(line) {
            scalar = Some(indent(line));
        }
    }
    let Some(at) = at else {
        return Vec::new();
    };
    let mut entries = Vec::new();
    let mut in_env = false;
    let mut entry_indent: Option<usize> = None;
    let mut scalar: Option<usize> = None;
    for line in &jobs_block[at + 1..] {
        if let Some(opened) = scalar {
            if !meaningful(line) || indent(line) > opened {
                continue;
            }
            scalar = None;
        }
        if !meaningful(line) {
            continue;
        }
        if indent(line) <= 2 {
            break;
        }
        if indent(line) == 4 {
            in_env = line.trim_end() == "    env:";
            entry_indent = None;
        } else if in_env {
            let level = *entry_indent.get_or_insert(indent(line));
            if indent(line) == level {
                entries.push(line.trim().to_owned());
            }
        }
        if opens_block_scalar(line) {
            scalar = Some(indent(line));
        }
    }
    entries
}

/// Every value `scripts/gate.sh` assigns a step command variable (`NAME_CMD=...`) at the
/// top level, by variable, in order: the value with its quotes taken off (`"..."`,
/// `'...'`, or a bare word), a trailing comment left off.
pub fn gate_command_assignments(gate: &str) -> BTreeMap<String, Vec<String>> {
    let mut found: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for line in gate.lines() {
        let Some((name, rest)) = line.split_once('=') else {
            continue;
        };
        if !(name.ends_with("_CMD")
            && name
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_'))
        {
            continue;
        }
        let value = match rest.chars().next() {
            Some(quote @ ('"' | '\'')) => {
                rest[1..].split(quote).next().unwrap_or_default().to_owned()
            }
            _ => rest
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .to_owned(),
        };
        found.entry(name.to_owned()).or_default().push(value);
    }
    found
}

/// The step command variables (`$NAME_CMD`, `${NAME_CMD}`) `function`'s definition in
/// `scripts/gate.sh` names — from `function() {` to its closing `}`, on one line or
/// several. Empty when there is no such definition.
pub fn gate_function_commands(gate: &str, function: &str) -> Vec<String> {
    let body = gate_function_body(gate, function).unwrap_or_default();
    let mut names = Vec::new();
    let mut rest = body.as_str();
    while let Some(at) = rest.find('$') {
        rest = &rest[at + 1..];
        let name: String = rest
            .trim_start_matches('{')
            .chars()
            .take_while(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || *c == '_')
            .collect();
        if name.ends_with("_CMD") && !names.contains(&name) {
            names.push(name);
        }
    }
    names
}

/// Whether `function`'s definition in `scripts/gate.sh` ([`gate_function_body`]) calls
/// `callee` as a command of its own: some statement of it — a line, or a part of one
/// between `;`s, its opening `{` and closing `}` taken off — whose first word is `callee`.
/// A comment, the name inside a string or as another command's argument, a longer name
/// that starts with it, and a call in any other function's definition are not calls.
pub fn gate_function_calls(gate: &str, function: &str, callee: &str) -> bool {
    let Some(body) = gate_function_body(gate, function) else {
        return false;
    };
    body.lines()
        .flat_map(|line| line.split(';'))
        .any(|statement| {
            let statement = statement.trim();
            let statement = statement
                .strip_prefix('{')
                .unwrap_or(statement)
                .trim_start();
            let statement = statement.strip_suffix('}').unwrap_or(statement);
            statement.split_whitespace().next() == Some(callee)
        })
}

/// `function`'s definition in `scripts/gate.sh`: what follows `function()` — its opening
/// `{` included — to the line that closes it, a lone `}`, or to the end of the line for a
/// definition on one line ending in `}`. `None` when there is no such definition.
pub fn gate_function_body(gate: &str, function: &str) -> Option<String> {
    let opening = format!("{function}()");
    let mut body = String::new();
    let mut inside = false;
    for line in gate.lines() {
        if !inside {
            let Some(rest) = line.trim_start().strip_prefix(&opening) else {
                continue;
            };
            inside = true;
            body.push_str(rest);
            if rest.trim_end().ends_with('}') {
                break;
            }
            continue;
        }
        body.push('\n');
        body.push_str(line);
        if line.trim() == "}" {
            break;
        }
    }
    inside.then_some(body)
}

/// The body of `scripts/gate.sh`'s default dispatch arm, line by line after its `*)`.
const GATE_DEFAULT_ARM: [&str; 3] = [
    "echo \"unknown gate step: $SELECTED_STEP\" >&2",
    "exit 2",
    ";;",
];

/// `scripts/gate.sh`'s `--step` dispatch arms, as `(step, function)`. Every line between
/// `case "$SELECTED_STEP" in` and its `esac` is read: each is a plain arm, exactly
/// `name) run_x ;;` with a plain step name and a bare `run_*` call, or the one default
/// arm, `*)` and then exactly [`GATE_DEFAULT_ARM`], last. Anything else — another
/// spelling of an arm (arguments, a `;` before `;;`, no spaces), a trailing comment, an
/// arm across lines, an alternation (`a|b)`), a comment or blank line, a step named
/// twice, an arm after the default, a default with another body, no default, no `esac` —
/// is an `Err` rather than a guess, so a step cannot be hidden from the guards that read
/// this by writing its arm in a shape they skip.
pub fn gate_dispatch_arms(gate: &str) -> Result<Vec<(String, String)>, String> {
    let mut lines = gate
        .lines()
        .skip_while(|line| line.trim() != "case \"$SELECTED_STEP\" in");
    if lines.next().is_none() {
        return Err("no `case \"$SELECTED_STEP\" in`".to_owned());
    }
    let plain = |word: &str| {
        !word.is_empty()
            && word
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    };
    let mut arms: Vec<(String, String)> = Vec::new();
    // `None` before the default arm; `Some(n)` once inside it, `n` lines of its body read.
    let mut default: Option<usize> = None;
    for line in lines {
        let trimmed = line.trim();
        if let Some(read) = default {
            if read == GATE_DEFAULT_ARM.len() {
                return if trimmed == "esac" {
                    Ok(arms)
                } else {
                    Err(format!("a line after the default arm: {trimmed:?}"))
                };
            }
            if trimmed != GATE_DEFAULT_ARM[read] {
                return Err(format!(
                    "the default arm's line {} is {trimmed:?}, not {:?}",
                    read + 1,
                    GATE_DEFAULT_ARM[read]
                ));
            }
            default = Some(read + 1);
            continue;
        }
        if trimmed == "*)" {
            default = Some(0);
            continue;
        }
        if trimmed == "esac" {
            return Err("the dispatch has no default arm".to_owned());
        }
        let arm = trimmed
            .strip_suffix(" ;;")
            .and_then(|body| body.split_once(") "))
            .filter(|(name, call)| {
                plain(name)
                    && call.starts_with("run_")
                    && plain(call)
                    && trimmed == format!("{name}) {call} ;;")
            });
        let Some((name, call)) = arm else {
            return Err(format!("a line that is no plain dispatch arm: {trimmed:?}"));
        };
        if arms.iter().any(|(step, _)| step == name) {
            return Err(format!("the step `{name}` has two arms"));
        }
        arms.push((name.to_owned(), call.to_owned()));
    }
    Err("the dispatch never reaches `esac`".to_owned())
}

/// The `run_*` functions `scripts/gate.sh` calls when run with no arguments: the full
/// gate, the merge bar. Read from the script's tail — everything after the `--step`
/// block's closing `fi`, up to `finish` — counting a call at the top level or in the
/// full branch of the one conditional the tail may hold (`if [ "$FAST" -eq 0 ]; then`),
/// and not in its `else`, which is the day loop's. Any other line there is an `Err`
/// rather than a guess, so a new conditional cannot hide a step from this reading.
pub fn gate_full_sequence(gate: &str) -> Result<BTreeSet<String>, String> {
    let mut lines = gate
        .lines()
        .skip_while(|line| line.trim() != "if [ -n \"$SELECTED_STEP\" ]; then");
    if lines.next().is_none() {
        return Err("no `if [ -n \"$SELECTED_STEP\" ]; then` block".to_owned());
    }
    let mut depth = 1usize;
    for line in lines.by_ref() {
        let trimmed = line.trim();
        if trimmed.starts_with("if ") {
            depth += 1;
        } else if trimmed == "fi" {
            depth -= 1;
            if depth == 0 {
                break;
            }
        }
    }
    if depth != 0 {
        return Err("the `--step` block never closes".to_owned());
    }

    #[derive(PartialEq)]
    enum Branch {
        Top,
        Full,
        Fast,
    }
    let mut branch = Branch::Top;
    let mut full = BTreeSet::new();
    let mut finished = false;
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        match trimmed {
            "if [ \"$FAST\" -eq 0 ]; then" if branch == Branch::Top => branch = Branch::Full,
            "else" if branch == Branch::Full => branch = Branch::Fast,
            "fi" if branch != Branch::Top => branch = Branch::Top,
            "finish" if branch == Branch::Top => {
                finished = true;
                break;
            }
            call if call.starts_with("run_")
                && call.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') =>
            {
                if branch != Branch::Fast {
                    full.insert(call.to_owned());
                }
            }
            other => return Err(format!("a line the reading does not know: {other:?}")),
        }
    }
    if !finished {
        return Err("the full sequence never reaches `finish`".to_owned());
    }
    Ok(full)
}

/// What is wrong with a directory of embedded fonts holding `files`, against `roster`: each
/// row a font file and the licence file it ships under. A file the roster does not name —
/// another font, a stray — is a font nobody decided to embed; a roster font missing is a
/// row nobody is keeping; and a font whose licence file is gone ships without the text its
/// licence requires beside it. Empty when the directory is exactly the roster.
pub fn embedded_font_violations(files: &[String], roster: &[(&str, &str)]) -> Vec<String> {
    let named = |file: &str| {
        roster
            .iter()
            .any(|(font, licence)| *font == file || *licence == file)
    };
    let mut violations: Vec<String> = files
        .iter()
        .filter(|file| !named(file))
        .map(|file| format!("`{file}` is not on the roster of embedded fonts and licences"))
        .collect();
    for (font, licence) in roster {
        if !files.iter().any(|file| file == font) {
            violations.push(format!("the roster names `{font}`, which is not there"));
        }
        if !files.iter().any(|file| file == licence) {
            violations.push(format!(
                "`{font}` has no licence beside it: `{licence}` is gone"
            ));
        }
    }
    violations
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
