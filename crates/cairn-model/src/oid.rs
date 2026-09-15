use std::fmt;

/// A git object id: the hash itself, fixed width, never on the heap.
///
/// Cairn keeps ids as the raw digest rather than re-exporting `gix`'s
/// `ObjectId`: the UI must be able to name a commit without linking the engine,
/// and plain bytes need no dependency to hold. Comparing two ids compares
/// bytes that are already wherever the id is — a lane scan asking "is this the
/// commit I am waiting for?" never chases a pointer and never allocates.
///
/// Both widths git defines are held: 20 bytes of SHA-1 or 32 of SHA-256. The
/// width travels with the bytes, so a SHA-1 is never mistaken for a SHA-256
/// whose tail happens to be zero, and the two never compare equal.
///
/// Text is produced on demand and borrowed from the caller's own frame:
/// [`Oid::hex`] and [`Oid::short`] write into an [`OidHex`] buffer, which is
/// what lets a list render a column of abbreviations per frame without
/// allocating one string per row.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Oid {
    /// The digest, left-aligned. Everything past `width` is zero, so the
    /// derived byte comparison reads only what was written.
    bytes: [u8; Oid::MAX_BYTES],
    /// Which of git's two hashes this is. Ordered after the bytes so that
    /// comparison is a byte comparison first and a discriminant second.
    width: Width,
}

/// The widths git defines, as the number of bytes in the digest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Width {
    Sha1 = 20,
    Sha256 = 32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OidParseError {
    /// Git object ids are 40 hex characters (SHA-1) or 64 (SHA-256).
    BadLength(usize),
    /// A digest, as raw bytes, is 20 (SHA-1) or 32 (SHA-256) of them.
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
    /// The widest digest an `Oid` holds, in bytes: SHA-256.
    const MAX_BYTES: usize = 32;
    /// The widest digest an `Oid` prints as, in hex characters.
    const MAX_HEX: usize = Self::MAX_BYTES * 2;
    /// How many hex characters [`Oid::short`] shows.
    const SHORT_HEX: usize = 7;

    /// Parse hex text, in either case: 40 characters for SHA-1, 64 for
    /// SHA-256. Anything else is rejected rather than truncated.
    pub fn parse(hex: &str) -> Result<Self, OidParseError> {
        let width = match hex.len() {
            40 => Width::Sha1,
            64 => Width::Sha256,
            other => return Err(OidParseError::BadLength(other)),
        };
        let mut bytes = [0u8; Self::MAX_BYTES];
        for (byte, pair) in bytes.iter_mut().zip(hex.as_bytes().chunks_exact(2)) {
            // `chunks_exact(2)` yields pairs; the fallback keeps the match
            // total without an index that could panic.
            let [high, low] = pair else {
                return Err(OidParseError::NotHex);
            };
            *byte = (nibble(*high)? << 4) | nibble(*low)?;
        }
        Ok(Self { bytes, width })
    }

    /// Take a digest as git itself stores it: 20 bytes or 32.
    ///
    /// This is the cheap boundary. A caller holding raw hash bytes — which is
    /// what a repository hands over — never has to render them to hex and read
    /// them back to cross the seam.
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

    /// The digest as git stores it: 20 bytes for SHA-1, 32 for SHA-256.
    pub fn as_bytes(&self) -> &[u8] {
        let width = self.width as usize;
        self.bytes.get(..width).unwrap_or(&self.bytes)
    }

    /// The full lowercase hex form, written into a buffer the caller owns.
    pub fn hex(&self) -> OidHex {
        OidHex::of(self.as_bytes(), Self::MAX_HEX)
    }

    /// The abbreviation a UI shows in a list. Not guaranteed unique in the
    /// repository; never use it to look an object back up.
    pub fn short(&self) -> OidHex {
        OidHex::of(self.as_bytes(), Self::SHORT_HEX)
    }
}

/// The hex text of an [`Oid`], held inline rather than on the heap.
///
/// Returned by [`Oid::hex`] and [`Oid::short`]. The digits live in the
/// caller's own frame, so naming a commit in text costs no allocation;
/// [`OidHex::as_str`] borrows from the buffer for as long as it is held.
#[derive(Clone, Copy)]
pub struct OidHex {
    digits: [u8; Oid::MAX_HEX],
    len: u8,
}

impl OidHex {
    /// Write `digest` out as hex, stopping once `wanted` characters exist.
    fn of(digest: &[u8], wanted: usize) -> Self {
        let mut digits = [0u8; Oid::MAX_HEX];
        let mut written = 0usize;
        for (pair, &byte) in digits.chunks_exact_mut(2).zip(digest) {
            if written >= wanted {
                break;
            }
            // `chunks_exact_mut(2)` yields pairs; the fallback keeps the match
            // total without an index that could panic.
            let [high, low] = pair else { break };
            *high = hex_digit(byte >> 4);
            *low = hex_digit(byte);
            written += 2;
        }
        let len = written.min(wanted).min(Oid::MAX_HEX);
        Self {
            digits,
            len: u8::try_from(len).unwrap_or(0),
        }
    }

    /// The characters, borrowed from this buffer.
    pub fn as_str(&self) -> &str {
        // Every byte written is an ASCII hex digit and `len` never passes what
        // was written, so neither step can fail. The fallback is here because
        // a git client may not panic on a path a user can reach, and it is
        // pinned by `every_width_round_trips_through_hex`.
        self.digits
            .get(..usize::from(self.len))
            .and_then(|written| std::str::from_utf8(written).ok())
            .unwrap_or("")
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

/// One hex character for the low four bits of `value`, lowercase.
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
        // A 40-BYTE string that is not 40 characters: the hex check is what
        // rejects it, not the length check.
        let wide = "é".repeat(20);
        assert_eq!(wide.len(), 40);
        assert_eq!(Oid::parse(&wide), Err(OidParseError::NotHex));
    }

    /// The width is carried, not inferred from the bytes: a SHA-1 and the
    /// SHA-256 whose first twenty bytes match it and whose tail is zero are
    /// different ids, and a fixed-width representation that dropped the width
    /// would fail exactly here.
    #[test]
    fn a_sha1_is_never_a_zero_padded_sha256() {
        let short = Oid::parse(&"ab".repeat(20)).unwrap();
        let long = Oid::parse(&format!("{}{}", "ab".repeat(20), "00".repeat(12))).unwrap();
        assert_ne!(short, long);
        assert_ne!(short.hex().as_str(), long.hex().as_str());
        assert_eq!(short.hex().as_str().len(), 40);
        assert_eq!(long.hex().as_str().len(), 64);
        // ... and neither may hash into the other's bucket by accident.
        use std::collections::HashSet;
        let both: HashSet<Oid> = [short, long].into_iter().collect();
        assert_eq!(both.len(), 2);
    }

    /// Both directions of the byte boundary, at both widths — this is the path
    /// the engine uses, and it never goes near hex.
    #[test]
    fn raw_digests_cross_in_both_directions() {
        for hex in [SHA1, SHA256] {
            let parsed = Oid::parse(hex).unwrap();
            let from_bytes = Oid::from_bytes(parsed.as_bytes()).unwrap();
            assert_eq!(parsed, from_bytes);
            assert_eq!(from_bytes.hex().as_str(), hex);
        }
        assert_eq!(
            Oid::from_bytes(&[0u8; 21]),
            Err(OidParseError::BadByteCount(21))
        );
        assert_eq!(Oid::from_bytes(&[]), Err(OidParseError::BadByteCount(0)));
    }

    /// `as_str` has a fallback for a state construction forbids; this is what
    /// says the fallback is never the answer. Every byte of every width, at
    /// both lengths `Oid` produces, has to come back as readable hex.
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

    /// Ordering and hashing are what the lane assigner runs on, so they must be
    /// total and agree with the text form rather than with the padding.
    #[test]
    fn ids_order_by_their_digest() {
        let low = Oid::parse(&format!("00{}", "ff".repeat(19))).unwrap();
        let high = Oid::parse(&format!("01{}", "00".repeat(19))).unwrap();
        assert!(low < high, "ordering does not follow the digest");
        assert_eq!(low.cmp(&low), std::cmp::Ordering::Equal);
    }
}
