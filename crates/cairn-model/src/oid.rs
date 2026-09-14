use std::fmt;

/// A git object id, stored as validated lowercase hex.
///
/// Cairn keeps ids as text at the boundary rather than re-exporting `gix`'s
/// `ObjectId`: the UI must be able to name a commit without linking the engine.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Oid(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OidParseError {
    /// Git object ids are 40 hex characters (SHA-1) or 64 (SHA-256).
    BadLength(usize),
    NotHex,
}

impl fmt::Display for OidParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadLength(n) => write!(f, "object id must be 40 or 64 hex characters, got {n}"),
            Self::NotHex => f.write_str("object id contains a non-hex character"),
        }
    }
}

impl std::error::Error for OidParseError {}

impl Oid {
    pub fn parse(hex: &str) -> Result<Self, OidParseError> {
        if hex.len() != 40 && hex.len() != 64 {
            return Err(OidParseError::BadLength(hex.len()));
        }
        if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(OidParseError::NotHex);
        }
        Ok(Self(hex.to_ascii_lowercase()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The abbreviation a UI shows in a list. Not guaranteed unique in the
    /// repository; never use it to look an object back up.
    pub fn short(&self) -> &str {
        &self.0[..7]
    }
}

impl fmt::Display for Oid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA1: &str = "0123456789abcdef0123456789abcdef01234567";

    #[test]
    fn parses_sha1_and_normalises_case() {
        let oid = Oid::parse(&SHA1.to_ascii_uppercase()).unwrap();
        assert_eq!(oid.as_str(), SHA1);
        assert_eq!(oid.short(), "0123456");
    }

    #[test]
    fn parses_sha256() {
        assert!(Oid::parse(&"a".repeat(64)).is_ok());
    }

    #[test]
    fn rejects_wrong_length_and_non_hex() {
        assert_eq!(Oid::parse("abc"), Err(OidParseError::BadLength(3)));
        assert_eq!(Oid::parse(&"z".repeat(40)), Err(OidParseError::NotHex));
    }
}
