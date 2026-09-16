use std::fmt;

/// A SHA-1 or SHA-256 object id; a SHA-1 never equals a zero-extended SHA-256.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Oid {
    /// Left-aligned, zero past `width`, so the derived `Eq` reads only what was written.
    bytes: [u8; Oid::MAX_BYTES],
    /// Declared after `bytes` so the derived `Ord` compares the digest first.
    width: Width,
}

/// Digest width; the discriminant is the byte count `as_bytes` slices by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Width {
    Sha1 = 20,
    Sha256 = 32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OidParseError {
    BadLength(usize),
    BadByteCount(usize),
    NotHex,
}

impl fmt::Display for OidParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadLength(n) => write!(f, "object id must be 40 or 64 hex characters, got {n}"),
            Self::BadByteCount(n) => write!(f, "object id must be 20 or 32 bytes, got {n}"),
            Self::NotHex => f.write_str("object id contains a non-hex character"),
        }
    }
}

impl std::error::Error for OidParseError {}

impl Oid {
    const MAX_BYTES: usize = 32;
    const MAX_HEX: usize = Self::MAX_BYTES * 2;
    const SHORT_HEX: usize = 7;

    /// Parses 40 or 64 hex characters, in either case.
    pub fn parse(hex: &str) -> Result<Self, OidParseError> {
        let width = match hex.len() {
            40 => Width::Sha1,
            64 => Width::Sha256,
            other => return Err(OidParseError::BadLength(other)),
        };
        let mut bytes = [0u8; Self::MAX_BYTES];
        for (byte, pair) in bytes.iter_mut().zip(hex.as_bytes().chunks_exact(2)) {
            // `chunks_exact(2)` always yields pairs; the arm avoids indexing.
            let [high, low] = pair else {
                return Err(OidParseError::NotHex);
            };
            *byte = (nibble(*high)? << 4) | nibble(*low)?;
        }
        Ok(Self { bytes, width })
    }

    /// Takes a digest as git stores it: 20 bytes or 32.
    pub fn from_bytes(digest: &[u8]) -> Result<Self, OidParseError> {
        let width = match digest.len() {
            20 => Width::Sha1,
            32 => Width::Sha256,
            other => return Err(OidParseError::BadByteCount(other)),
        };
        let mut bytes = [0u8; Self::MAX_BYTES];
        let Some(slot) = bytes.get_mut(..digest.len()) else {
            return Err(OidParseError::BadByteCount(digest.len()));
        };
        slot.copy_from_slice(digest);
        Ok(Self { bytes, width })
    }

    pub fn as_bytes(&self) -> &[u8] {
        // Empty on the unreachable miss: the whole buffer would pass a SHA-1 off as a SHA-256.
        self.bytes.get(..self.width as usize).unwrap_or_default()
    }

    /// The full hex form, lowercase.
    pub fn hex(&self) -> OidHex {
        OidHex::of(self.as_bytes(), Self::MAX_HEX)
    }

    /// A 7-character abbreviation. Not unique; never look an object up by it.
    pub fn short(&self) -> OidHex {
        OidHex::of(self.as_bytes(), Self::SHORT_HEX)
    }
}

/// The hex text of an [`Oid`], held inline.
#[derive(Clone, Copy)]
pub struct OidHex {
    /// Zero past `len`, so equal abbreviations hold equal buffers.
    digits: [u8; Oid::MAX_HEX],
    len: usize,
}

impl OidHex {
    /// Writes `digest` as hex, stopping once `wanted` characters exist.
    fn of(digest: &[u8], wanted: usize) -> Self {
        let mut digits = [0u8; Oid::MAX_HEX];
        let mut written = 0usize;
        for (pair, &byte) in digits.chunks_exact_mut(2).zip(digest) {
            if written >= wanted {
                break;
            }
            // `chunks_exact_mut(2)` always yields pairs; the arm avoids indexing.
            let [high, low] = pair else { break };
            *high = hex_digit(byte >> 4);
            *low = hex_digit(byte);
            written += 2;
        }
        // An odd `wanted` overshoots by one digit: cut to it and blank the tail.
        let len = written.min(wanted).min(Oid::MAX_HEX);
        if let Some(tail) = digits.get_mut(len..) {
            tail.fill(0);
        }
        Self { digits, len }
    }

    pub fn as_str(&self) -> &str {
        // Empty rather than a panic; unreachable.
        self.digits
            .get(..self.len)
            .and_then(|written| std::str::from_utf8(written).ok())
            .unwrap_or("")
    }
}

impl PartialEq for OidHex {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for OidHex {}

impl std::hash::Hash for OidHex {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

impl PartialEq<str> for OidHex {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for OidHex {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl AsRef<str> for OidHex {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl From<OidHex> for String {
    fn from(hex: OidHex) -> Self {
        hex.as_str().to_owned()
    }
}

/// What a `'static` text sink such as a UI label takes.
impl From<OidHex> for std::borrow::Cow<'static, str> {
    fn from(hex: OidHex) -> Self {
        Self::Owned(hex.into())
    }
}

impl fmt::Display for OidHex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl fmt::Debug for OidHex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.as_str())
    }
}

impl fmt::Display for Oid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.hex().as_str())
    }
}

impl fmt::Debug for Oid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Oid({})", self.hex().as_str())
    }
}

/// The lowercase hex character for the low four bits of `value`.
const fn hex_digit(value: u8) -> u8 {
    match value & 0x0f {
        digit @ 0..=9 => b'0' + digit,
        digit => b'a' + digit - 10,
    }
}

/// The four bits one hex character stands for, in either case.
fn nibble(digit: u8) -> Result<u8, OidParseError> {
    match digit {
        b'0'..=b'9' => Ok(digit - b'0'),
        b'a'..=b'f' => Ok(digit - b'a' + 10),
        b'A'..=b'F' => Ok(digit - b'A' + 10),
        _ => Err(OidParseError::NotHex),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA1: &str = "0123456789abcdef0123456789abcdef01234567";
    const SHA256: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn parses_sha1_and_normalises_case() {
        let oid = Oid::parse(&SHA1.to_ascii_uppercase()).unwrap();
        assert_eq!(oid.hex().as_str(), SHA1);
        assert_eq!(oid.short().as_str(), "0123456");
        assert_eq!(oid.to_string(), SHA1);
    }

    #[test]
    fn parses_sha256() {
        let oid = Oid::parse(&SHA256.to_ascii_uppercase()).unwrap();
        assert_eq!(oid.hex().as_str(), SHA256);
        assert_eq!(oid.as_bytes().len(), 32);
    }

    #[test]
    fn rejects_wrong_length_and_non_hex() {
        assert_eq!(Oid::parse("abc"), Err(OidParseError::BadLength(3)));
        assert_eq!(Oid::parse(&"z".repeat(40)), Err(OidParseError::NotHex));
        assert_eq!(
            Oid::parse(&"a".repeat(41)),
            Err(OidParseError::BadLength(41))
        );
        // 40 bytes but not 40 characters: the hex check rejects it, not length.
        let wide = "é".repeat(20);
        assert_eq!(wide.len(), 40);
        assert_eq!(Oid::parse(&wide), Err(OidParseError::NotHex));
    }

    #[test]
    fn a_length_either_side_of_each_width_is_rejected() {
        for length in [32usize, 39, 41, 63, 65] {
            assert_eq!(
                Oid::parse(&"a".repeat(length)),
                Err(OidParseError::BadLength(length)),
                "{length} hex characters was accepted"
            );
        }
        assert!(Oid::parse(&"a".repeat(40)).is_ok());
        assert!(Oid::parse(&"a".repeat(64)).is_ok());
    }

    #[test]
    fn a_single_bad_character_is_rejected_at_either_half_of_a_pair() {
        for index in [0usize, 1, 20, 21, 38, 39] {
            let mut hex: Vec<char> = SHA1.chars().collect();
            hex[index] = 'g';
            let spoiled: String = hex.into_iter().collect();
            assert_eq!(spoiled.len(), 40);
            assert_eq!(
                Oid::parse(&spoiled),
                Err(OidParseError::NotHex),
                "a bad character at index {index} was accepted, giving a \
                 well-formed but wrong id"
            );
        }
    }

    #[test]
    fn a_sha1_is_never_a_zero_padded_sha256() {
        let short = Oid::parse(&"ab".repeat(20)).unwrap();
        let long = Oid::parse(&format!("{}{}", "ab".repeat(20), "00".repeat(12))).unwrap();
        assert_ne!(short, long);
        assert_ne!(short.hex().as_str(), long.hex().as_str());
        assert_eq!(short.hex().as_str().len(), 40);
        assert_eq!(long.hex().as_str().len(), 64);
        // Separate map keys follows from `Eq`, not `Hash`: collisions are legal.
        use std::collections::HashSet;
        let both: HashSet<Oid> = [short, long].into_iter().collect();
        assert_eq!(both.len(), 2);
    }

    #[test]
    fn raw_digests_cross_in_both_directions() {
        for hex in [SHA1, SHA256] {
            let parsed = Oid::parse(hex).unwrap();
            let from_bytes = Oid::from_bytes(parsed.as_bytes()).unwrap();
            assert_eq!(parsed, from_bytes);
            assert_eq!(from_bytes.hex().as_str(), hex);
        }
        // The whole band is rejected: accepting 24 would drop four bytes.
        for count in [0usize, 19, 21, 24, 31, 33] {
            assert_eq!(
                Oid::from_bytes(&vec![0u8; count]),
                Err(OidParseError::BadByteCount(count)),
                "a {count}-byte digest was accepted"
            );
        }
    }

    /// Pins `as_str`'s empty fallback as unreachable at either width.
    #[test]
    fn every_width_round_trips_through_hex() {
        for width in [20usize, 32] {
            let digest: Vec<u8> = (0..width).map(|n| (n * 7 + 3) as u8).collect();
            let oid = Oid::from_bytes(&digest).unwrap();
            let hex = oid.hex();
            assert_eq!(hex.as_str().len(), width * 2, "hex was truncated");
            assert!(
                hex.as_str().bytes().all(|b| b.is_ascii_hexdigit()),
                "a non-hex character was written: {hex:?}"
            );
            assert_eq!(Oid::parse(hex.as_str()).unwrap(), oid);
            let short = oid.short();
            assert_eq!(short.as_str(), &hex.as_str()[..Oid::SHORT_HEX]);
        }
    }

    #[test]
    fn ids_order_by_their_digest() {
        let low = Oid::parse(&format!("00{}", "ff".repeat(19))).unwrap();
        let high = Oid::parse(&format!("01{}", "00".repeat(19))).unwrap();
        assert!(low < high, "ordering does not follow the digest");
        assert_eq!(low.cmp(&low), std::cmp::Ordering::Equal);
    }

    #[test]
    fn ordering_compares_the_digest_before_the_width() {
        let sha1 = Oid::parse(&"ff".repeat(20)).unwrap();
        let sha256 = Oid::parse(&format!("{}{}", "ff".repeat(20), "00".repeat(12))).unwrap();
        let low_sha256 = Oid::parse(&"00".repeat(32)).unwrap();
        assert!(sha1 < sha256, "a SHA-1 lost to the SHA-256 that extends it");
        assert!(
            low_sha256 < sha1,
            "the width was compared before the digest"
        );
        assert_eq!(
            sha1 < sha256,
            sha1.to_string() < sha256.to_string(),
            "ordering stopped agreeing with the text form"
        );
    }

    #[test]
    fn every_text_form_renders_the_id_it_names() {
        let sha256 = Oid::parse(SHA256).unwrap();
        assert_eq!(sha256.to_string(), SHA256, "Display truncated a SHA-256");
        assert_eq!(format!("{sha256:?}"), format!("Oid({SHA256})"));

        let sha1 = Oid::parse(SHA1).unwrap();
        assert_eq!(sha1.hex().to_string(), SHA1, "OidHex lost its Display");
        assert_eq!(sha1.short().to_string(), "0123456");
        assert_eq!(format!("{:?}", sha1.short()), "\"0123456\"");

        assert_eq!(
            OidParseError::BadByteCount(24).to_string(),
            "object id must be 20 or 32 bytes, got 24"
        );
    }

    /// Caught by: comparing buffers rather than the text, or the full id equalling its abbreviation.
    #[test]
    fn id_text_compares_by_its_digits() {
        let a = Oid::parse(&format!("{}{}", "0123456", "f".repeat(33))).unwrap();
        let b = Oid::parse(&format!("{}{}", "0123456", "0".repeat(33))).unwrap();
        assert_eq!(a.short(), b.short(), "equal abbreviations compared unequal");
        assert_ne!(a.hex(), b.hex());
        assert_ne!(a.hex(), a.short(), "an id equalled its own abbreviation");

        let sha1 = Oid::parse(SHA1).unwrap();
        assert!(sha1.hex() == *SHA1);
        assert!(sha1.hex() == SHA1);
        assert!(sha1.short() == "0123456");
        assert!(
            sha1.short() != "012345",
            "a prefix of the text compared equal"
        );
        assert!(sha1.hex() != SHA256);

        use std::collections::HashSet;
        let texts: HashSet<OidHex> = [a.short(), b.short(), a.hex(), b.hex()]
            .into_iter()
            .collect();
        assert_eq!(texts.len(), 3, "equal text did not hash as one key");
    }

    #[test]
    fn id_text_reads_as_a_str_wherever_one_is_taken() {
        fn length(text: impl AsRef<str>) -> usize {
            text.as_ref().len()
        }
        let sha256 = Oid::parse(SHA256).unwrap();
        assert_eq!(length(sha256.hex()), 64);
        assert_eq!(length(sha256.short()), 7);
        assert_eq!(sha256.short().as_ref(), "0123456");
    }

    /// The owned forms a `'static` text sink takes, with no `.to_string()` at the call site.
    #[test]
    fn id_text_becomes_owned_text_without_a_detour() {
        fn sink(text: impl Into<std::borrow::Cow<'static, str>>) -> String {
            text.into().into_owned()
        }
        let sha1 = Oid::parse(SHA1).unwrap();
        assert_eq!(sink(sha1.short()), "0123456");
        assert_eq!(sink(sha1.hex()), SHA1);
        assert_eq!(String::from(sha1.hex()), SHA1);
    }

    #[test]
    fn an_abbreviation_leaves_nothing_behind_the_length_it_reports() {
        let a = Oid::parse(&format!("{}{}", "0123456", "f".repeat(33))).unwrap();
        let b = Oid::parse(&format!("{}{}", "0123456", "0".repeat(33))).unwrap();
        assert_eq!(a.short().as_str(), b.short().as_str());
        assert_eq!(
            a.short().digits,
            b.short().digits,
            "the buffer carried a digit past the length it reports"
        );
    }
}
