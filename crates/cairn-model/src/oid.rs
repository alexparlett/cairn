use std::fmt;

/// A git object id: the hash itself, fixed width, never on the heap.
///
/// The raw digest rather than a re-export of `gix`'s `ObjectId`, because the UI
/// must be able to name a commit without linking the engine.
///
/// Both widths git defines are held: 20 bytes of SHA-1 or 32 of SHA-256. The
/// width travels with the bytes, so a SHA-1 is never mistaken for a SHA-256
/// whose tail happens to be zero, and the two never compare equal.
///
/// Text is produced on demand into a stack buffer the caller owns
/// ([`Oid::hex`], [`Oid::short`]), so naming an id costs no allocation; drawing
/// one still copies out of that buffer — see [`OidHex`].
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Oid {
    /// The digest, left-aligned. Everything past `width` is zero, so the
    /// derived byte comparison reads only what was written.
    bytes: [u8; Oid::MAX_BYTES],
    /// Which of git's two hashes this is. Declared after the bytes so that
    /// comparison reads the digest first and the width second.
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
            // `chunks_exact(2)` always yields pairs; the arm keeps the match
            // total without an index that could panic.
            let [high, low] = pair else {
                return Err(OidParseError::NotHex);
            };
            *byte = (nibble(*high)? << 4) | nibble(*low)?;
        }
        Ok(Self { bytes, width })
    }

    /// Take a digest as git itself stores it: 20 bytes or 32. This is the
    /// boundary the engine uses — raw bytes never go via hex to cross the seam.
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
        // The slice always exists: `width` is one of two values, both inside
        // the array. Empty is the fail-safe fallback — handing back the whole
        // buffer would pass a SHA-1 off as a SHA-256 and look up the wrong
        // object.
        self.bytes.get(..self.width as usize).unwrap_or_default()
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
/// Returned by [`Oid::hex`] and [`Oid::short`]. The digits live in the caller's
/// own frame, and [`OidHex::as_str`] borrows from that buffer for as long as it
/// is held; a caller needing the text to outlive the frame copies it out.
#[derive(Clone, Copy)]
pub struct OidHex {
    /// The characters, zero past `len`, so the buffer is canonical: a future
    /// `PartialEq` or `Hash` over it cannot tell two identical abbreviations
    /// apart by what an earlier write left behind.
    digits: [u8; Oid::MAX_HEX],
    len: usize,
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
            // `chunks_exact_mut(2)` always yields pairs; the arm keeps the
            // match total without an index that could panic.
            let [high, low] = pair else { break };
            *high = hex_digit(byte >> 4);
            *low = hex_digit(byte);
            written += 2;
        }
        // A byte is two characters, so an odd `wanted` leaves one written past
        // it. Cut to what was asked for and blank the rest, rather than carry a
        // digit nothing reports.
        let len = written.min(wanted).min(Oid::MAX_HEX);
        if let Some(tail) = digits.get_mut(len..) {
            tail.fill(0);
        }
        Self { digits, len }
    }

    /// The characters, borrowed from this buffer.
    pub fn as_str(&self) -> &str {
        // Neither step can fail: every byte written is an ASCII hex digit and
        // `len` never passes what was written. The fallback is here because a
        // git client may not panic on a path a user can reach; that it is never
        // the answer is pinned by `every_width_round_trips_through_hex`.
        self.digits
            .get(..self.len)
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
        // A 40-BYTE string that is not 40 characters: the hex check rejects
        // it, not the length check.
        let wide = "é".repeat(20);
        assert_eq!(wide.len(), 40);
        assert_eq!(Oid::parse(&wide), Err(OidParseError::NotHex));
    }

    /// Caught by: a length rule that accepts a RANGE ending at 40 or beginning
    /// at 64 — it passes a fixture of 3 and 41 while silently zero-padding a
    /// short id or truncating a long one.
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

    /// One mistyped character in an otherwise valid id. Caught by: dropping
    /// either half of the hex-pair check — a fixture where EVERY character is
    /// invalid cannot tell the two halves apart, so this one puts the bad
    /// character on each side of the pair in turn.
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

    /// Caught by: dropping the carried width. A SHA-1 and the SHA-256 whose
    /// first twenty bytes match it and whose tail is zero are different ids.
    #[test]
    fn a_sha1_is_never_a_zero_padded_sha256() {
        let short = Oid::parse(&"ab".repeat(20)).unwrap();
        let long = Oid::parse(&format!("{}{}", "ab".repeat(20), "00".repeat(12))).unwrap();
        assert_ne!(short, long);
        assert_ne!(short.hex().as_str(), long.hex().as_str());
        assert_eq!(short.hex().as_str().len(), 40);
        assert_eq!(long.hex().as_str().len(), 64);
        // ... and both stand as separate keys where the assigner indexes
        // commits by id. This follows from `Eq`, not `Hash` — a hash collision
        // is legal — so it pins the map, not the hasher.
        use std::collections::HashSet;
        let both: HashSet<Oid> = [short, long].into_iter().collect();
        assert_eq!(both.len(), 2);
    }

    /// Both directions of the byte boundary, at both widths: the path the
    /// engine uses, which never goes near hex.
    #[test]
    fn raw_digests_cross_in_both_directions() {
        for hex in [SHA1, SHA256] {
            let parsed = Oid::parse(hex).unwrap();
            let from_bytes = Oid::from_bytes(parsed.as_bytes()).unwrap();
            assert_eq!(parsed, from_bytes);
            assert_eq!(from_bytes.hex().as_str(), hex);
        }
        // The whole band between the two widths is rejected, not just the byte
        // after SHA-1: accepting 24 would take a digest and drop four bytes.
        for count in [0usize, 19, 21, 24, 31, 33] {
            assert_eq!(
                Oid::from_bytes(&vec![0u8; count]),
                Err(OidParseError::BadByteCount(count)),
                "a {count}-byte digest was accepted"
            );
        }
    }

    /// The twin for `as_str`'s fallback: every byte of every width, at both
    /// lengths `Oid` produces, comes back as readable hex, so the fallback is
    /// never the answer.
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

    /// The lane assigner runs on ordering and hashing, so they must be total
    /// and agree with the text form rather than with the padding.
    #[test]
    fn ids_order_by_their_digest() {
        let low = Oid::parse(&format!("00{}", "ff".repeat(19))).unwrap();
        let high = Oid::parse(&format!("01{}", "00".repeat(19))).unwrap();
        assert!(low < high, "ordering does not follow the digest");
        assert_eq!(low.cmp(&low), std::cmp::Ordering::Equal);
    }

    /// Caught by: comparing the width before the digest — every SHA-1 would
    /// sort before every SHA-256 whatever their bytes, and the order would stop
    /// agreeing with the text. Nothing but the field order says this.
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

    /// Each text form pinned to its own output — `Display` for errors and logs,
    /// `Debug` for a failed assertion. Caught by: either rendering nothing at
    /// all, which every other test would tolerate.
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

    /// Caught by: leaving a digit past the reported length. `short()` writes
    /// whole pairs and then cuts to seven, so two abbreviations that read the
    /// same would otherwise differ by an eighth digit nobody can see.
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
