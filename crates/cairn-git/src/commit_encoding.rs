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
//! ISO-8859-1 is decoded as itself (every byte the code point of its own number), and every
//! other encoding through `encoding_rs` (user-approved 2026-10-03), whose tables are the
//! WHATWG encoding standard's rather than glibc's. Every 1- and 2-byte sequence (and
//! GB18030's 4-byte range from `0x81308130` to `0x8439fe39`) was decoded both ways
//! (2026-10-03, glibc's iconv against `encoding_rs` 0.8.41), and where the two read a
//! sequence as different characters, or where one refuses what the other reads, this module
//! takes glibc's side wherever a [`Quirk`] can: the WHATWG standard reads US-ASCII and
//! ISO-8859-1 as windows-1252 and ISO-8859-9 and ISO-8859-11 as windows-1254 and
//! windows-874, decodes the bytes a windows code page leaves undefined, and maps Shift_JIS's
//! `0x5c` and `0x7e`, six JIS X 0208 codes, two KOI8-U box drawings and two Mac Roman
//! characters as Microsoft's or its own tables do. What no quirk reaches — bytes one side
//! refuses in the multi-byte encodings — is a known limit in `docs/systems/diff.md`.

use encoding_rs::Encoding;

/// How a commit's bytes become the characters git shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CommitEncoding {
    /// UTF-8, or bytes git prints unconverted: invalid sequences read as U+FFFD, as a UTF-8
    /// terminal draws them.
    Utf8,
    /// ISO-8859-1: every byte is the code point of its own number.
    Latin1,
    /// An encoding `encoding_rs` decodes, with what makes its answer glibc's.
    Iconv {
        encoding: &'static Encoding,
        quirk: Quirk,
    },
}

/// Where glibc's iconv reads an encoding otherwise than the WHATWG table `encoding_rs`
/// follows, and how to read it as glibc does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Quirk {
    /// The two agree on every sequence both read.
    None,
    /// A windows code page: the WHATWG table reads a byte it leaves undefined as the C1
    /// control of the same number, which glibc refuses — so a C1 control decoded is a
    /// refusal.
    UndefinedRefused,
    /// ISO-8859-9 or ISO-8859-11, which the WHATWG standard reads as windows-1254 or
    /// windows-874: glibc keeps `0x80..=0x9f` as the C1 controls.
    C1Kept,
    /// TIS-620, which the WHATWG standard reads as windows-874: glibc refuses
    /// `0x80..=0x9f`.
    C1Refused,
    /// Characters the WHATWG table reads otherwise than glibc, each turned into glibc's.
    Remapped(&'static [(char, char)]),
    /// glibc's `SHIFT_JIS`, which is JIS's: what [`Quirk::MicrosoftShiftJis`] refuses, the
    /// rows JIS X 0208 leaves empty and Microsoft fills (NEC's row 13, rows 85-94, the
    /// user-defined and IBM rows from lead byte `0xf0`), and [`SHIFT_JIS_AS_GLIBC`].
    JisShiftJis,
    /// glibc's `CP932`: the WHATWG table, but for the lone bytes `0x80`, `0xa0` and
    /// `0xfd..=0xff`, which glibc refuses.
    MicrosoftShiftJis,
    /// glibc's `GB18030`: lone `0x80` and `0xff` refused, and [`GB18030_AS_GLIBC`]'s codes
    /// read as glibc reads them.
    Gb18030,
}

/// Names matched as iconv matches them, ignoring case.
const UTF_8: &[&str] = &["UTF-8", "UTF8"];
/// US-ASCII: a byte above `0x7f` fails iconv, and git then prints the bytes as UTF-8 would
/// read them — so UTF-8 is the answer either way. The WHATWG standard would read windows-1252.
const ASCII: &[&str] = &[
    "ASCII",
    "US-ASCII",
    "ANSI_X3.4-1968",
    "ISO646-US",
    "US",
    "CP367",
    "IBM367",
    "CSASCII",
    "ISO-IR-6",
    "ISO_646.IRV:1991",
];
/// ISO-8859-1's names glibc's iconv accepts, measured with `iconv -f <name>`, and `latin-1`,
/// which iconv refuses and git's `fallback_encoding` (utf8.c) renames to `ISO-8859-1`.
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
/// ISO-8859-9's WHATWG labels, which name windows-1254 there.
const ISO_8859_9: &[&str] = &[
    "ISO-8859-9",
    "ISO8859-9",
    "ISO88599",
    "ISO_8859-9",
    "ISO_8859-9:1989",
    "ISO-IR-148",
    "L5",
    "LATIN5",
    "CSISOLATIN5",
];
/// ISO-8859-11's WHATWG labels, which name windows-874 there.
const ISO_8859_11: &[&str] = &["ISO-8859-11", "ISO8859-11", "ISO885911"];
/// TIS-620, which names windows-874 in the WHATWG standard.
const TIS_620: &[&str] = &["TIS-620"];
/// Microsoft's Shift_JIS, which the WHATWG table is; every other Shift_JIS name is JIS's.
const CP932: &[&str] = &["CP932", "WINDOWS-31J", "MS932"];
/// KOI8-U as glibc has it; the WHATWG `koi8-u` is KOI8-RU.
const KOI8_U: &[&str] = &["KOI8-U"];

/// Names glibc's iconv knows that the WHATWG standard does not, as the label it does know.
const ALIASES: &[(&str, &str)] = &[
    ("CP932", "windows-31j"),
    ("EUCJP", "euc-jp"),
    ("CP936", "gbk"),
    ("EUC-CN", "gb2312"),
    ("EUCCN", "gb2312"),
    ("CP949", "euc-kr"),
    ("UHC", "euc-kr"),
    ("CP874", "windows-874"),
    ("MS-ANSI", "windows-1252"),
];

/// glibc's `SHIFT_JIS`: JIS X 0201 for `0x5c` and `0x7e`, and JIS X 0208's own characters
/// for `0x8160`, `0x8161`, `0x817c`, `0x8191`, `0x8192` and `0x81ca`, where the WHATWG
/// table has Microsoft's.
const SHIFT_JIS_AS_GLIBC: &[(char, char)] = &[
    ('\\', '¥'),
    ('~', '‾'),
    ('～', '〜'),
    ('∥', '‖'),
    ('－', '−'),
    ('￠', '¢'),
    ('￡', '£'),
    ('￢', '¬'),
];
/// glibc's `GB18030` reads these codes as private-use points where the WHATWG table has
/// assigned characters (measured over every 1- and 2-byte code and the 4-byte codes from
/// `0x81308130` to `0x8439fe39`).
const GB18030_AS_GLIBC: &[(&[u8], char)] = &[
    (b"\xa3\xa0", '\u{e5e5}'),
    (b"\x82\x35\x90\x37", '\u{e81e}'),
    (b"\x82\x35\x90\x38", '\u{e826}'),
    (b"\x82\x35\x90\x39", '\u{e82b}'),
    (b"\x82\x35\x91\x30", '\u{e82c}'),
    (b"\x82\x35\x91\x31", '\u{e832}'),
    (b"\x82\x35\x91\x32", '\u{e843}'),
    (b"\x82\x35\x91\x33", '\u{e854}'),
    (b"\x82\x35\x91\x34", '\u{e864}'),
    (b"\x84\x31\x82\x36", '\u{e78d}'),
    (b"\x84\x31\x82\x37", '\u{e78f}'),
    (b"\x84\x31\x82\x38", '\u{e78e}'),
    (b"\x84\x31\x82\x39", '\u{e790}'),
    (b"\x84\x31\x83\x30", '\u{e791}'),
    (b"\x84\x31\x83\x31", '\u{e792}'),
    (b"\x84\x31\x83\x32", '\u{e793}'),
    (b"\x84\x31\x83\x33", '\u{e794}'),
    (b"\x84\x31\x83\x34", '\u{e795}'),
    (b"\x84\x31\x83\x35", '\u{e796}'),
];

/// Whether a Shift_JIS byte string holds a lone byte glibc refuses — `0x80`, `0xa0`,
/// `0xfd..=0xff` — or, under JIS's table (`jis`), a two-byte code in a row JIS X 0208 does
/// not fill: lead byte `0x87` (rows 13-14) or `0xeb..=0xfc` (rows 85 and on).
fn shift_jis_refused(bytes: &[u8], jis: bool) -> bool {
    let mut at = 0;
    while let Some(&byte) = bytes.get(at) {
        match byte {
            0x00..=0x7f | 0xa1..=0xdf => at += 1,
            0x80 | 0xa0 | 0xfd..=0xff => return true,
            _ if jis && (byte == 0x87 || (0xeb..=0xfc).contains(&byte)) => return true,
            _ => at += 2,
        }
    }
    false
}

/// A GB18030 byte string in its codes — one byte, two, or four when the second is a digit —
/// as glibc reads them, or `None` where glibc refuses.
fn gb18030(bytes: &[u8]) -> Option<String> {
    let mut out = String::with_capacity(bytes.len());
    let mut at = 0;
    while let Some(&byte) = bytes.get(at) {
        let width = match byte {
            0x00..=0x7f => 1,
            0x80 | 0xff => return None,
            _ if bytes.get(at + 1).is_some_and(|next| next.is_ascii_digit()) => 4,
            _ => 2,
        };
        let code = bytes.get(at..at + width)?;
        match GB18030_AS_GLIBC.iter().find(|(known, _)| *known == code) {
            Some((_, glibc)) => out.push(*glibc),
            None => out.push_str(
                &encoding_rs::GB18030.decode_without_bom_handling_and_without_replacement(code)?,
            ),
        }
        at += width;
    }
    Some(out)
}

/// glibc's `EUC-JP`: the same six JIS X 0208 codes, at `0xa1c1` and on.
const EUC_JP_AS_GLIBC: &[(char, char)] = &[
    ('～', '〜'),
    ('∥', '‖'),
    ('－', '−'),
    ('￠', '¢'),
    ('￡', '£'),
    ('￢', '¬'),
];
/// glibc's `KOI8-U` has box drawings at `0xae` and `0xbe`.
const KOI8_U_AS_GLIBC: &[(char, char)] = &[('ў', '╝'), ('Ў', '╬')];
/// glibc's `MACINTOSH`: `0xc6` is the Greek capital delta, `0xf0` a private-use point.
const MACINTOSH_AS_GLIBC: &[(char, char)] = &[('∆', 'Δ'), ('\u{f8ff}', '\u{e01e}')];

fn is_c1(c: char) -> bool {
    ('\u{80}'..='\u{9f}').contains(&c)
}

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
        if named(UTF_8) || named(ASCII) {
            return Self::Utf8;
        }
        if named(LATIN_1) {
            return Self::Latin1;
        }
        let whatwg = ALIASES
            .iter()
            .find(|(name, _)| label.eq_ignore_ascii_case(name.as_bytes()))
            .map_or(label, |(_, known)| known.as_bytes());
        let Some(encoding) = Encoding::for_label_no_replacement(whatwg) else {
            return Self::Utf8;
        };
        // UTF-16 commits are not git's to convert sensibly, and `x-user-defined` is no
        // charset iconv has: their bytes as UTF-8, as git prints what it cannot convert.
        if [
            encoding_rs::UTF_8,
            encoding_rs::UTF_16LE,
            encoding_rs::UTF_16BE,
            encoding_rs::X_USER_DEFINED,
        ]
        .contains(&encoding)
        {
            return Self::Utf8;
        }
        let quirk = if (encoding == encoding_rs::WINDOWS_1254 && named(ISO_8859_9))
            || (encoding == encoding_rs::WINDOWS_874 && named(ISO_8859_11))
        {
            Quirk::C1Kept
        } else if encoding == encoding_rs::WINDOWS_874 && named(TIS_620) {
            Quirk::C1Refused
        } else if encoding.name().starts_with("windows-") && encoding.is_single_byte() {
            Quirk::UndefinedRefused
        } else if encoding == encoding_rs::SHIFT_JIS && named(CP932) {
            Quirk::MicrosoftShiftJis
        } else if encoding == encoding_rs::SHIFT_JIS {
            Quirk::JisShiftJis
        } else if encoding == encoding_rs::GB18030 {
            Quirk::Gb18030
        } else if encoding == encoding_rs::EUC_JP {
            Quirk::Remapped(EUC_JP_AS_GLIBC)
        } else if encoding == encoding_rs::KOI8_U && named(KOI8_U) {
            Quirk::Remapped(KOI8_U_AS_GLIBC)
        } else if encoding == encoding_rs::MACINTOSH {
            Quirk::Remapped(MACINTOSH_AS_GLIBC)
        } else {
            Quirk::None
        };
        let read = Self::Iconv { encoding, quirk };
        // One refused sequence anywhere in the object, and git prints all of it unconverted.
        if read.decoded(data).is_some() {
            read
        } else {
            Self::Utf8
        }
    }

    /// `bytes`, one field of the commit, as characters.
    pub(crate) fn text(self, bytes: &[u8]) -> String {
        self.decoded(bytes)
            .unwrap_or_else(|| String::from_utf8_lossy(bytes).into_owned())
    }

    /// `bytes` as characters, or `None` where glibc's iconv would refuse them.
    fn decoded(self, bytes: &[u8]) -> Option<String> {
        match self {
            Self::Utf8 => Some(String::from_utf8_lossy(bytes).into_owned()),
            Self::Latin1 => Some(encoding_rs::mem::decode_latin1(bytes).into_owned()),
            Self::Iconv { encoding, quirk } => {
                let refused = match quirk {
                    Quirk::C1Refused => bytes.iter().any(|b| (0x80..=0x9f).contains(b)),
                    Quirk::JisShiftJis => shift_jis_refused(bytes, true),
                    Quirk::MicrosoftShiftJis => shift_jis_refused(bytes, false),
                    Quirk::Gb18030 => return gb18030(bytes),
                    Quirk::None | Quirk::UndefinedRefused | Quirk::C1Kept | Quirk::Remapped(_) => {
                        false
                    }
                };
                if refused {
                    return None;
                }
                let text = encoding.decode_without_bom_handling_and_without_replacement(bytes)?;
                let table = match quirk {
                    Quirk::JisShiftJis => SHIFT_JIS_AS_GLIBC,
                    Quirk::Remapped(table) => table,
                    Quirk::None
                    | Quirk::C1Refused
                    | Quirk::UndefinedRefused
                    | Quirk::C1Kept
                    | Quirk::MicrosoftShiftJis
                    | Quirk::Gb18030 => &[],
                };
                match quirk {
                    Quirk::None | Quirk::C1Refused | Quirk::MicrosoftShiftJis | Quirk::Gb18030 => {
                        Some(text.into_owned())
                    }
                    Quirk::UndefinedRefused => {
                        (!text.chars().any(is_c1)).then(|| text.into_owned())
                    }
                    // A single-byte encoding: one character per byte, in order.
                    Quirk::C1Kept => Some(
                        text.chars()
                            .zip(bytes)
                            .map(|(c, byte)| {
                                if (0x80..=0x9f).contains(byte) {
                                    char::from(*byte)
                                } else {
                                    c
                                }
                            })
                            .collect(),
                    ),
                    Quirk::Remapped(_) | Quirk::JisShiftJis => Some(
                        text.chars()
                            .map(|c| {
                                table
                                    .iter()
                                    .find(|(from, _)| *from == c)
                                    .map_or(c, |(_, to)| *to)
                            })
                            .collect(),
                    ),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(label: &str) -> CommitEncoding {
        CommitEncoding::of(Some(label.as_bytes()), b"author x\n\nmessage")
    }

    /// windows-1252 against iconv's own output for every byte it defines in `0x80..=0x9f`
    /// (`iconv -f WINDOWS-1252 -t UTF-16BE`), and each byte it leaves undefined refused.
    /// Caught by: the WHATWG reading of an undefined byte as a C1 control kept.
    #[test]
    fn windows_1252_reads_its_high_bytes_as_iconv_does() {
        let undefined = [0x81, 0x8d, 0x8f, 0x90, 0x9d];
        let defined: Vec<u8> = (0x80..=0x9f).filter(|b| !undefined.contains(b)).collect();
        let iconv = "€‚ƒ„…†‡ˆ‰Š‹ŒŽ‘’“”•–—˜™š›œžŸ";
        assert_eq!(named("windows-1252").text(&defined), iconv);
        for byte in undefined {
            assert_eq!(
                CommitEncoding::of(Some(b"cp1252"), &[b'a', byte]),
                CommitEncoding::Utf8,
                "{byte:#x}"
            );
        }
    }

    /// ISO-8859-1 keeps the C1 controls windows-1252 replaces, and both agree above them.
    /// Caught by: decoding ISO-8859-1 as windows-1252, as the WHATWG encoding standard does.
    #[test]
    fn latin_1_is_not_windows_1252() {
        assert_eq!(named("latin1").text(b"\x80\xe9\xff"), "\u{80}éÿ");
        assert_eq!(named("windows-1252").text(b"\x80\xe9\xff"), "€éÿ");
    }

    /// A name is matched as iconv matches it, glibc's own names reach the encoding the
    /// WHATWG standard calls otherwise, and an undefined byte anywhere sends the whole
    /// object back to its bytes. Caught by: matching case, accepting a name iconv refuses
    /// for ISO-8859-1, reading US-ASCII as windows-1252, or checking only one field.
    #[test]
    fn the_encoding_is_the_one_git_would_convert_from() {
        assert_eq!(CommitEncoding::of(None, b"x"), CommitEncoding::Utf8);
        assert_eq!(named("latin1"), CommitEncoding::Latin1);
        assert_eq!(named("iso-8859-1"), CommitEncoding::Latin1);
        assert_eq!(named("Latin-1"), CommitEncoding::Latin1);
        assert_eq!(named("us-ascii"), CommitEncoding::Utf8);
        assert_eq!(named("iso-8859_1"), CommitEncoding::Utf8);
        assert_eq!(named("x-no-such-encoding"), CommitEncoding::Utf8);
        assert_eq!(
            named("ISO-2022-KR"),
            CommitEncoding::Utf8,
            "WHATWG's replacement"
        );
        assert_eq!(named("UTF-16"), CommitEncoding::Utf8);
        for (glibc, whatwg) in [
            ("CP932", encoding_rs::SHIFT_JIS),
            ("eucJP", encoding_rs::EUC_JP),
            ("CP936", encoding_rs::GBK),
            ("Shift_JIS", encoding_rs::SHIFT_JIS),
            ("KOI8-R", encoding_rs::KOI8_R),
        ] {
            assert!(
                matches!(named(glibc), CommitEncoding::Iconv { encoding, .. } if encoding == whatwg),
                "{glibc}: {:?}",
                named(glibc)
            );
        }
        assert_eq!(
            CommitEncoding::of(Some(b"windows-1252"), b"author \x81\n\nmessage"),
            CommitEncoding::Utf8
        );
    }

    /// Each quirk against glibc's reading of the same bytes (`iconv -f <name> -t UTF-8`).
    /// Caught by: any quirk dropped, which leaves the WHATWG reading.
    #[test]
    fn each_encoding_reads_as_glibc_reads_it() {
        for (label, bytes, glibc) in [
            (
                "Shift_JIS",
                &b"\x5c\x7e\x81\x60\x81\x61\x81\x7c\x81\x91\x81\x92\x81\xca\x95\x5c"[..],
                "¥‾〜‖−¢£¬表",
            ),
            ("CP932", b"\x5c\x7e\x81\x60\x87\x40", "\\~～①"),
            (
                "EUC-JP",
                b"\x5c\xa1\xc1\xa1\xc2\xa1\xdd\xa1\xf1\xa1\xf2\xa2\xcc",
                "\\〜‖−¢£¬",
            ),
            ("ISO-8859-9", b"\x80\x9f\xfd", "\u{80}\u{9f}ı"),
            ("ISO-8859-11", b"\x80\xa1", "\u{80}ก"),
            ("KOI8-U", b"\xae\xbe\xa6", "╝╬і"),
            ("macintosh", b"\xc6\x8e", "Δé"),
            ("KOI8-R", b"\xf0\xd2\xc9", "При"),
        ] {
            assert_eq!(named(label).text(bytes), glibc, "{label}");
        }
        assert_eq!(
            named("GB18030").text(b"\xa3\xa0\x84\x31\x82\x36\xd6\xd0\x81\x30\x85\x38"),
            "\u{e5e5}\u{e78d}中µ"
        );
        for (label, refused) in [
            ("Shift_JIS", &b"\x87\x40"[..]),
            ("Shift_JIS", b"\xfa\x40"),
            ("Shift_JIS", b"\x80"),
            ("CP932", b"\xfd"),
            ("GB18030", b"\x80"),
            ("TIS-620", b"\x80"),
            ("windows-874", b"\x81"),
            ("windows-1251", b"\x98"),
        ] {
            assert_eq!(
                CommitEncoding::of(Some(label.as_bytes()), refused),
                CommitEncoding::Utf8,
                "{label} {refused:x?}"
            );
        }
    }
}
