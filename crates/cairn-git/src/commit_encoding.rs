//! A commit's text in the characters git shows it in (user decision, 2026-10-03: re-encode
//! like git).
//!
//! A commit may name the encoding its text was written in, in an `encoding` header. Before
//! it prints any of the commit — names, addresses and message alike — git converts the WHOLE
//! object from that encoding to its output encoding with iconv; with no header it assumes
//! UTF-8 and converts nothing. When the conversion fails — a byte the encoding does not
//! define, or an encoding iconv does not know — git prints the object's bytes as they are,
//! every field of it, not just the one that failed. Measured against git 2.56 on glibc,
//! by `the_text_of_an_encoded_commit_is_the_text_git_prints`.
//!
//! `i18n.logOutputEncoding` (and `i18n.commitEncoding`, its fallback) changes only which
//! BYTES carry those characters: measured, a UTF-8 commit's `é` leaves git as `0xe9` under
//! `ISO-8859-1` and as `0xc3 0xa9` without it, and a character the output encoding lacks
//! leaves as the commit's own bytes. Cairn draws characters, never bytes, so it decodes to
//! them once and the setting has nothing left to change.
//!
//! Decoded without a dependency: ISO-8859-1 maps each byte to the code point of the same
//! number, and windows-1252 differs from it in one table of 27 characters. Any other
//! encoding a header names is read as UTF-8 — what git does for one iconv does not know,
//! and a divergence from git for one it does (Shift_JIS, EUC-JP, KOI8-R, ...), stated in
//! `docs/systems/diff.md`'s known limits.

/// How a commit's bytes become the characters git shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CommitEncoding {
    /// UTF-8, or bytes git prints unconverted: invalid sequences read as U+FFFD, as a UTF-8
    /// terminal draws them.
    Utf8,
    /// ISO-8859-1: every byte is the code point of its own number.
    Latin1,
    /// windows-1252: ISO-8859-1 but for [`WINDOWS_1252_HIGH`].
    Windows1252,
}

/// Names git converts from for each encoding Cairn decodes, matched as iconv matches them —
/// ignoring case: those glibc's iconv accepts, measured with `iconv -f <name>`, and
/// `latin-1`, which iconv refuses and git's `fallback_encoding` (utf8.c) renames to
/// `ISO-8859-1` before asking again. A spelling neither accepts (`windows1252`,
/// `iso-8859_1`) is not here, because git cannot convert from it either.
const UTF_8: &[&str] = &["UTF-8", "UTF8"];
const LATIN_1: &[&str] = &[
    "ISO-8859-1",
    "ISO8859-1",
    "ISO88591",
    "ISO_8859-1",
    "ISO_8859-1:1987",
    "8859_1",
    "LATIN1",
    "L1",
    "IBM819",
    "CP819",
    "CSISOLATIN1",
    "ISO-IR-100",
    "LATIN-1",
];
const WINDOWS_1252: &[&str] = &["WINDOWS-1252", "CP1252", "MS-ANSI"];

/// The bytes windows-1252 leaves undefined, which iconv refuses to convert.
const WINDOWS_1252_UNDEFINED: [u8; 5] = [0x81, 0x8d, 0x8f, 0x90, 0x9d];

/// windows-1252's characters for `0x80..=0x9f`, where ISO-8859-1 has its C1 controls; the
/// undefined bytes hold their C1 control, and are refused before this is read.
const WINDOWS_1252_HIGH: [char; 32] = [
    '\u{20ac}', '\u{0081}', '\u{201a}', '\u{0192}', '\u{201e}', '\u{2026}', '\u{2020}', '\u{2021}',
    '\u{02c6}', '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}', '\u{008d}', '\u{017d}', '\u{008f}',
    '\u{0090}', '\u{2018}', '\u{2019}', '\u{201c}', '\u{201d}', '\u{2022}', '\u{2013}', '\u{2014}',
    '\u{02dc}', '\u{2122}', '\u{0161}', '\u{203a}', '\u{0153}', '\u{009d}', '\u{017e}', '\u{0178}',
];

impl CommitEncoding {
    /// How git reads `commit`, already `decoded`.
    pub(crate) fn of_commit(commit: &gix::Commit<'_>, decoded: &gix::objs::CommitRef<'_>) -> Self {
        Self::of(decoded.encoding.map(|label| &**label), &commit.data)
    }

    /// How git reads the commit object `data`, whose `encoding` header is `label`.
    pub(crate) fn of(label: Option<&[u8]>, data: &[u8]) -> Self {
        let Some(label) = label else {
            return Self::Utf8;
        };
        let named = |names: &[&str]| {
            names
                .iter()
                .any(|name| label.eq_ignore_ascii_case(name.as_bytes()))
        };
        if named(UTF_8) {
            Self::Utf8
        } else if named(LATIN_1) {
            Self::Latin1
        } else if named(WINDOWS_1252) {
            // One undefined byte anywhere in the object fails git's conversion of all of it.
            if data
                .iter()
                .any(|byte| WINDOWS_1252_UNDEFINED.contains(byte))
            {
                Self::Utf8
            } else {
                Self::Windows1252
            }
        } else {
            Self::Utf8
        }
    }

    /// `bytes`, one field of the commit, as characters.
    pub(crate) fn text(self, bytes: &[u8]) -> String {
        match self {
            Self::Utf8 => String::from_utf8_lossy(bytes).into_owned(),
            Self::Latin1 => bytes.iter().map(|byte| char::from(*byte)).collect(),
            Self::Windows1252 => bytes
                .iter()
                .map(|byte| match byte {
                    0x80..=0x9f => WINDOWS_1252_HIGH
                        .get(usize::from(byte - 0x80))
                        .copied()
                        .unwrap_or(char::REPLACEMENT_CHARACTER),
                    _ => char::from(*byte),
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The table against iconv's own output for every byte it defines in `0x80..=0x9f`
    /// (`iconv -f WINDOWS-1252 -t UTF-16BE`). Caught by: a transposed entry.
    #[test]
    fn windows_1252_reads_its_high_bytes_as_iconv_does() {
        let defined: Vec<u8> = (0x80..=0x9f)
            .filter(|byte| !WINDOWS_1252_UNDEFINED.contains(byte))
            .collect();
        let iconv = "€‚ƒ„…†‡ˆ‰Š‹ŒŽ‘’“”•–—˜™š›œžŸ";
        assert_eq!(CommitEncoding::Windows1252.text(&defined), iconv);
    }

    /// ISO-8859-1 keeps the C1 controls windows-1252 replaces, and both agree above them.
    /// Caught by: decoding ISO-8859-1 as windows-1252, as the WHATWG encoding standard does.
    #[test]
    fn latin_1_is_not_windows_1252() {
        assert_eq!(CommitEncoding::Latin1.text(b"\x80\xe9\xff"), "\u{80}éÿ");
        assert_eq!(CommitEncoding::Windows1252.text(b"\x80\xe9\xff"), "€éÿ");
    }

    /// A name is matched as iconv matches it, and an undefined byte anywhere sends the whole
    /// object back to its bytes. Caught by: matching case, accepting a name iconv refuses,
    /// or checking only the field being read.
    #[test]
    fn the_encoding_is_the_one_git_would_convert_from() {
        let plain = b"author x\n\nmessage";
        assert_eq!(CommitEncoding::of(None, plain), CommitEncoding::Utf8);
        assert_eq!(
            CommitEncoding::of(Some(b"latin1"), plain),
            CommitEncoding::Latin1
        );
        assert_eq!(
            CommitEncoding::of(Some(b"iso-8859-1"), plain),
            CommitEncoding::Latin1
        );
        assert_eq!(
            CommitEncoding::of(Some(b"Latin-1"), plain),
            CommitEncoding::Latin1
        );
        assert_eq!(
            CommitEncoding::of(Some(b"iso-8859_1"), plain),
            CommitEncoding::Utf8
        );
        assert_eq!(
            CommitEncoding::of(Some(b"cp1252"), plain),
            CommitEncoding::Windows1252
        );
        assert_eq!(
            CommitEncoding::of(Some(b"windows1252"), plain),
            CommitEncoding::Utf8
        );
        assert_eq!(
            CommitEncoding::of(Some(b"windows-1252"), b"author \x81\n\nmessage"),
            CommitEncoding::Utf8
        );
        assert_eq!(
            CommitEncoding::of(Some(b"Shift_JIS"), plain),
            CommitEncoding::Utf8
        );
    }
}
