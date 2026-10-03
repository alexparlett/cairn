//! Reading the user's configuration the way git reads it: the last value a key is given
//! across every file gix loaded, and git's own parsers for what it holds.
//!
//! What the changes query honours of the configuration the user's own `git log` and
//! `git show` read — `diff.renames` and `diff.renameLimit` (`renames.rs`),
//! `diff.ignoreSubmodules` and `submodule.<name>.ignore` (`submodules.rs`), and
//! `log.showRoot` — is read here, from the configuration gix loaded when the repository
//! was opened, and a value git refuses is [`Error::InvalidConfig`], because the user's
//! own `git log` refuses to answer on it too.

use gix::bstr::BString;

use crate::Error;

pub(super) fn invalid(key: &str, value: Option<BString>) -> Error {
    Error::InvalidConfig {
        key: key.to_owned(),
        value: value.map_or_else(
            || "(no value)".to_owned(),
            |value| String::from_utf8_lossy(&value).into_owned(),
        ),
    }
}

/// The last value `<section>[.<subsection>].<key>` is given, across every file in git's
/// order: `Some(None)` for the bare key, and `None` when it is not set at all. Section and
/// key match whatever their case, as git's do; a subsection matches exactly, and a key in
/// another subsection (`[diff "x"]` for `diff.<key>`) is a different key.
pub(super) fn last_value(
    file: &gix::config::File,
    section: &str,
    subsection: Option<&[u8]>,
    key: &str,
) -> Option<Option<BString>> {
    file.sections_by_name(section)?
        .filter(|found| found.header().subsection_name().map(|name| name.as_ref()) == subsection)
        .filter_map(|found| found.value_implicit(key))
        .last()
}

/// `git_config_bool`: the bare key is true; a value is `git_parse_maybe_bool` — empty is
/// false, then the words, then any integer git accepts, non-zero meaning true. `None` for
/// anything git refuses.
pub(super) fn parse_bool(value: Option<&[u8]>) -> Option<bool> {
    let Some(value) = value else {
        return Some(true);
    };
    if value.is_empty() {
        return Some(false);
    }
    for word in [&b"true"[..], b"yes", b"on"] {
        if value.eq_ignore_ascii_case(word) {
            return Some(true);
        }
    }
    for word in [&b"false"[..], b"no", b"off"] {
        if value.eq_ignore_ascii_case(word) {
            return Some(false);
        }
    }
    parse_int(value).map(|number| number != 0)
}

/// `git_parse_int`: C's `strtoimax` in base 0 — leading whitespace, a sign, `0x` for hex
/// and a leading `0` for octal — then nothing, or one of the unit suffixes `k`, `m`, `g`,
/// and the result within an `int`, whose smallest accepted value is `-INT_MAX`.
pub(super) fn parse_int(value: &[u8]) -> Option<i64> {
    let max = i64::from(i32::MAX);
    let mut rest = value;
    while let [first, tail @ ..] = rest
        && matches!(first, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
    {
        rest = tail;
    }
    let negative = match rest.first() {
        Some(b'-') => {
            rest = &rest[1..];
            true
        }
        Some(b'+') => {
            rest = &rest[1..];
            false
        }
        _ => false,
    };
    let hex_digits = rest
        .strip_prefix(b"0x")
        .or_else(|| rest.strip_prefix(b"0X"))
        .filter(|digits| digits.first().is_some_and(u8::is_ascii_hexdigit));
    let (radix, digits) = match hex_digits {
        Some(digits) => (16, digits),
        None if rest.first() == Some(&b'0') => (8, rest),
        None => (10, rest),
    };
    let length = digits
        .iter()
        .take_while(|byte| char::from(**byte).is_digit(radix))
        .count();
    if length == 0 {
        return None;
    }
    let (number, suffix) = digits.split_at(length);
    let mut magnitude: u128 = 0;
    for digit in number {
        let digit = char::from(*digit).to_digit(radix)?;
        // Far past an `int` already: git's `strtoimax` would report ERANGE or its range
        // check would refuse it, and either is a refusal.
        magnitude = magnitude
            .checked_mul(u128::from(radix))?
            .checked_add(u128::from(digit))?;
        if magnitude > u128::from(u64::MAX) {
            return None;
        }
    }
    let factor: u128 = match suffix {
        [] => 1,
        [unit] if unit.eq_ignore_ascii_case(&b'k') => 1 << 10,
        [unit] if unit.eq_ignore_ascii_case(&b'm') => 1 << 20,
        [unit] if unit.eq_ignore_ascii_case(&b'g') => 1 << 30,
        _ => return None,
    };
    let scaled = magnitude.checked_mul(factor)?;
    if scaled > max as u128 {
        return None;
    }
    let scaled = i64::try_from(scaled).ok()?;
    Some(if negative { -scaled } else { scaled })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `git_parse_int` over the forms `strtoimax` in base 0 accepts and the ones the range
    /// and suffix checks refuse.
    #[test]
    fn an_integer_reads_the_way_git_parse_int_does() {
        let cases: &[(&str, Option<i64>)] = &[
            ("1000", Some(1000)),
            ("+5", Some(5)),
            ("-5", Some(-5)),
            ("  \t12", Some(12)),
            ("0", Some(0)),
            ("010", Some(8)),
            ("0x1F", Some(31)),
            ("0X1f", Some(31)),
            ("1k", Some(1024)),
            ("2M", Some(2 << 20)),
            ("1g", Some(1 << 30)),
            ("2147483647", Some(2_147_483_647)),
            ("-2147483647", Some(-2_147_483_647)),
            ("2147483648", None),
            ("-2147483648", None),
            ("2g", None),
            ("", None),
            ("k", None),
            ("12 ", None),
            ("12kb", None),
            ("1.5", None),
            ("08", None),
            ("0x", None),
            ("0xg", None),
            ("ten", None),
            ("99999999999999999999999999", None),
        ];
        for (value, expected) in cases {
            assert_eq!(parse_int(value.as_bytes()), *expected, "{value:?}");
        }
    }

    /// `git_config_bool` over each shape: the bare key, the empty value, the words in any
    /// case, numbers, and what git refuses. Real git reading the same values is
    /// `the_root_commit_is_shown_as_log_show_root_says`, in `tests/diff/changes.rs`.
    #[test]
    fn a_boolean_reads_the_way_git_config_bool_does() {
        assert_eq!(parse_bool(None), Some(true), "the bare key");
        for value in ["true", "YES", "On", "1", "-1", "0x10", "1k"] {
            assert_eq!(parse_bool(Some(value.as_bytes())), Some(true), "{value}");
        }
        for value in ["", "false", "No", "OFF", "0", "00", "0k"] {
            assert_eq!(parse_bool(Some(value.as_bytes())), Some(false), "{value}");
        }
        for value in ["maybe", "true ", "1x", "08", "99999999999"] {
            assert_eq!(parse_bool(Some(value.as_bytes())), None, "{value}");
        }
    }

    /// The last value wins across sections, the key's case does not matter, the bare key
    /// is `Some(None)`, and a subsection is a key of its own.
    #[test]
    fn the_last_value_of_a_key_is_the_one_read() {
        let file = gix::config::File::try_from(
            "[diff]\n\trenames = false\n[Diff]\n\tRENAMES = copies\n\
             [diff \"x\"]\n\trenames = true\n[log]\n\tshowRoot\n\
             [submodule \"s\"]\n\tignore = none\n[submodule \"S\"]\n\tignore = all\n",
        )
        .unwrap();
        assert_eq!(
            last_value(&file, "diff", None, "renames"),
            Some(Some("copies".into()))
        );
        assert_eq!(
            last_value(&file, "diff", Some(b"x"), "renames"),
            Some(Some("true".into()))
        );
        assert_eq!(last_value(&file, "log", None, "showroot"), Some(None));
        assert_eq!(last_value(&file, "log", None, "missing"), None);
        assert_eq!(
            last_value(&file, "submodule", Some(b"s"), "ignore"),
            Some(Some("none".into())),
            "a subsection's name keeps its case"
        );
    }
}
