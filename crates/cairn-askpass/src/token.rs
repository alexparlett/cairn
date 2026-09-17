//! Fresh tokens, from the kernel's randomness.

use std::io::{self, Read};

use cairn_model::AskpassToken;

/// 256 bits, which is more than "unguessable by another user who cannot reach
/// the socket anyway" needs; hex, so it survives any environment.
const BYTES: usize = 32;

pub(crate) fn fresh() -> io::Result<AskpassToken> {
    let mut bytes = [0u8; BYTES];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    let mut hex = String::with_capacity(BYTES * 2);
    for byte in bytes {
        hex.push_str(&format!("{byte:02x}"));
    }
    Ok(AskpassToken::new(hex))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_long_hex_and_differ() {
        let a = fresh().unwrap();
        let b = fresh().unwrap();
        assert_eq!(a.as_str().len(), BYTES * 2);
        assert!(a.as_str().bytes().all(|b| b.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }
}
