//! Matchers and repository walking for Cairn's invariant guards.
//!
//! The assertions themselves live in `tests/invariants.rs`; this crate holds
//! the pieces they are built from so each matcher can be proven, in its own
//! unit test, to fire on the DISGUISED forms of what it forbids — an aliased
//! import, a fully-qualified path, a wrapped line. A matcher that only catches
//! the obvious spelling reports green while the invariant rots.

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
    let bytes = line.as_bytes();
    let mut from = 0;
    while let Some(offset) = line[from..].find(ident) {
        let start = from + offset;
        let end = start + ident.len();
        let before_ok = start == 0 || !is_ident_byte(bytes[start - 1]);
        let after_ok = end == bytes.len() || !is_ident_byte(bytes[end]);
        if before_ok && after_ok {
            return true;
        }
        from = end;
    }
    false
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
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
