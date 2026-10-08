//! A path as a patch spells it: git's `quote_c_style`, byte for byte (R2.5, L17e).
//!
//! `git apply` refuses a raw tab in a path line and reads a quoted name back through
//! `unquote_c_style`, so every path the emitter writes goes through here. The rules are
//! git's `cq_lookup` table in `quote.c` (read at v2.30.0 and v2.56.0, unchanged), under
//! `core.quotePath`'s default of true — the form `git diff` itself prints and every `git
//! apply` reads:
//!
//! - nothing in the name needs quoting: the name as it is, unquoted;
//! - otherwise the whole of it in double quotes, with `"` and `\` escaped by a backslash,
//!   BEL, BS, TAB, LF, VT, FF and CR as `\a`, `\b`, `\t`, `\n`, `\v`, `\f`, `\r`, every
//!   other control byte, DEL and every byte above `0x7f` as a three-digit octal escape —
//!   so a name that is not UTF-8 is spelled out byte by byte, and so is one that is.
//!
//! A space alone needs no quoting. `quote_two_c_style` quotes a prefix and a name together,
//! so `a/` goes inside the quotes: `"a/ta\tb"`.

/// What a byte becomes inside quotes: itself, a letter after a backslash, or an octal
/// escape. `None` for a byte that never needs quoting.
fn escape(byte: u8) -> Option<Escape> {
    match byte {
        0x07 => Some(Escape::Letter(b'a')),
        0x08 => Some(Escape::Letter(b'b')),
        b'\t' => Some(Escape::Letter(b't')),
        b'\n' => Some(Escape::Letter(b'n')),
        0x0b => Some(Escape::Letter(b'v')),
        0x0c => Some(Escape::Letter(b'f')),
        b'\r' => Some(Escape::Letter(b'r')),
        b'"' => Some(Escape::Letter(b'"')),
        b'\\' => Some(Escape::Letter(b'\\')),
        0x00..=0x1f | 0x7f..=0xff => Some(Escape::Octal),
        0x20..=0x7e => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Escape {
    Letter(u8),
    Octal,
}

/// `prefix` and `name` as `quote_two_c_style` writes them: together, unquoted when nothing
/// in either needs it, and otherwise inside one pair of quotes.
pub(crate) fn quoted_with_prefix(prefix: &[u8], name: &[u8]) -> Vec<u8> {
    let needs = prefix
        .iter()
        .chain(name)
        .any(|byte| escape(*byte).is_some());
    let mut out = Vec::with_capacity(prefix.len() + name.len() + 2);
    if !needs {
        out.extend_from_slice(prefix);
        out.extend_from_slice(name);
        return out;
    }
    out.push(b'"');
    for byte in prefix.iter().chain(name) {
        match escape(*byte) {
            None => out.push(*byte),
            Some(Escape::Letter(letter)) => {
                out.push(b'\\');
                out.push(letter);
            }
            Some(Escape::Octal) => {
                out.push(b'\\');
                out.push(b'0' + ((byte >> 6) & 0o3));
                out.push(b'0' + ((byte >> 3) & 0o7));
                out.push(b'0' + (byte & 0o7));
            }
        }
    }
    out.push(b'"');
    out
}

/// `name` as `quote_c_style` writes it.
pub(crate) fn quoted(name: &[u8]) -> Vec<u8> {
    quoted_with_prefix(b"", name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(bytes: Vec<u8>) -> String {
        String::from_utf8(bytes).expect("a quoted name is ASCII, or the name itself")
    }

    /// Each awkward name in the spelling `git diff` printed for it under git 2.56.0 and
    /// 2.30.9 (`docs/systems/diff.md`, "Path lines"); what proves these against git itself
    /// is `cairn-git`'s C3, which applies each with real `git apply`.
    #[test]
    fn each_awkward_name_is_spelled_as_git_spells_it() {
        for (name, spelled) in [
            (&b"plain/name.rs"[..], "a/plain/name.rs"),
            (b"sp ace", "a/sp ace"),
            (b"ta\tb", r#""a/ta\tb""#),
            (b"quo\"te", r#""a/quo\"te""#),
            (b"back\\slash", r#""a/back\\slash""#),
            (b"new\nline", r#""a/new\nline""#),
            (b"ctl\x01x", r#""a/ctl\001x""#),
            (b"bad\xffutf", r#""a/bad\377utf""#),
            ("ümlaut".as_bytes(), r#""a/\303\274mlaut""#),
            (b"del\x7f", r#""a/del\177""#),
            (b"bell\x07\x08\x0b\x0c\r", r#""a/bell\a\b\v\f\r""#),
        ] {
            assert_eq!(
                text(quoted_with_prefix(b"a/", name)),
                spelled,
                "{:?}",
                String::from_utf8_lossy(name)
            );
        }
    }

    /// Without a prefix, as `rename from` and `copy to` spell the name.
    #[test]
    fn a_name_alone_is_quoted_only_when_it_needs_to_be() {
        assert_eq!(text(quoted(b"sp ace")), "sp ace");
        assert_eq!(text(quoted(b"ta\tb")), r#""ta\tb""#);
        assert_eq!(text(quoted(b"")), "");
    }

    /// Every byte that git leaves alone is left alone, and every other is escaped: the whole
    /// table, so a byte moved from one class to the other is caught.
    #[test]
    fn only_printable_ascii_but_quote_and_backslash_goes_out_as_itself() {
        for byte in 0u8..=255 {
            let spelled = quoted(&[byte]);
            let plain = (0x20..=0x7e).contains(&byte) && byte != b'"' && byte != b'\\';
            if plain {
                assert_eq!(spelled, vec![byte], "{byte:#04x} was quoted");
            } else {
                assert_eq!(spelled.first(), Some(&b'"'), "{byte:#04x} was not quoted");
                assert_eq!(spelled.get(1), Some(&b'\\'), "{byte:#04x} was not escaped");
            }
        }
    }
}
