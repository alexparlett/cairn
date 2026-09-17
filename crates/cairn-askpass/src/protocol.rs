//! The wire format between the helper and the channel.
//!
//! One request per connection. The helper writes a greeting line, the token
//! line and the prompt bytes, then shuts its writing half; the channel reads
//! to that end, answers with one status line followed by the secret bytes (or
//! nothing), and closes. Bytes, not text: a prompt is whatever `git` or `ssh`
//! put on `argv`, and a secret is whatever the user typed.

use std::io::{self, Read, Write};

use cairn_model::{AskpassToken, Secret};

const HELLO: &[u8] = b"cairn-askpass/1\n";
const ANSWERED: &[u8] = b"answered\n";
const REFUSED: &[u8] = b"refused\n";

/// Longest request the channel reads: a prompt is one line from git or ssh.
pub(crate) const REQUEST_LIMIT: u64 = 64 * 1024;

/// Longest response the helper reads: a status line and one secret.
pub(crate) const RESPONSE_LIMIT: u64 = 64 * 1024;

/// What a helper asks for.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Request {
    pub(crate) token: AskpassToken,
    pub(crate) prompt: Vec<u8>,
}

/// The request was not one this protocol version understands.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Malformed;

pub(crate) fn write_request(
    to: &mut impl Write,
    token: &AskpassToken,
    prompt: &[u8],
) -> io::Result<()> {
    to.write_all(HELLO)?;
    to.write_all(token.as_str().as_bytes())?;
    to.write_all(b"\n")?;
    to.write_all(prompt)?;
    to.flush()
}

/// Reads to the end of `from`; the caller bounds it with [`REQUEST_LIMIT`].
pub(crate) fn read_request(mut from: impl Read) -> io::Result<Result<Request, Malformed>> {
    let mut bytes = Vec::new();
    from.read_to_end(&mut bytes)?;
    let Some(rest) = bytes.strip_prefix(HELLO) else {
        return Ok(Err(Malformed));
    };
    let Some(newline) = rest.iter().position(|byte| *byte == b'\n') else {
        return Ok(Err(Malformed));
    };
    let Ok(token) = std::str::from_utf8(&rest[..newline]) else {
        return Ok(Err(Malformed));
    };
    if token.is_empty() {
        return Ok(Err(Malformed));
    }
    Ok(Ok(Request {
        token: AskpassToken::new(token),
        prompt: rest[newline + 1..].to_vec(),
    }))
}

pub(crate) fn write_answer(to: &mut impl Write, secret: &Secret) -> io::Result<()> {
    to.write_all(ANSWERED)?;
    to.write_all(secret.expose_secret())?;
    to.flush()
}

pub(crate) fn write_refusal(to: &mut impl Write) -> io::Result<()> {
    to.write_all(REFUSED)?;
    to.flush()
}

/// The channel's answer, or `None` when it refused. Reads to the end of
/// `from`, bounded by [`RESPONSE_LIMIT`], into a buffer sized up front so the
/// bytes are never moved by a reallocation; the status line is drained out of
/// that same buffer and the rest becomes the [`Secret`], so the one
/// allocation that ever held the bytes is the one zeroed on drop.
pub(crate) fn read_response(mut from: impl Read) -> io::Result<Option<Secret>> {
    let mut bytes = Vec::with_capacity(usize::try_from(RESPONSE_LIMIT).unwrap_or(usize::MAX));
    from.read_to_end(&mut bytes)?;
    if !bytes.starts_with(ANSWERED) {
        // Whatever it was, it was not an answer; scrub it on the way out.
        let _scrubbed = Secret::new(bytes);
        return Ok(None);
    }
    bytes.drain(..ANSWERED.len());
    Ok(Some(Secret::new(bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token() -> AskpassToken {
        AskpassToken::new(format!("t-{}", std::process::id()))
    }

    #[test]
    fn a_request_round_trips_with_a_prompt_of_any_bytes() {
        let prompt = b"Password for 'https://x@host':\x00 with \xff bytes\n";
        let mut wire = Vec::new();
        write_request(&mut wire, &token(), prompt).unwrap();
        let read = read_request(wire.as_slice()).unwrap().unwrap();
        assert_eq!(read.token, token());
        assert_eq!(read.prompt, prompt);
    }

    #[test]
    fn an_empty_prompt_is_still_a_request() {
        let mut wire = Vec::new();
        write_request(&mut wire, &token(), b"").unwrap();
        let read = read_request(wire.as_slice()).unwrap().unwrap();
        assert_eq!(read.prompt, b"");
    }

    #[test]
    fn anything_but_the_greeting_is_malformed() {
        for wire in [
            b"".as_slice(),
            b"hello\n",
            b"cairn-askpass/2\ntoken\nprompt",
            b"cairn-askpass/1\n",
            b"cairn-askpass/1\ntoken-without-newline",
            b"cairn-askpass/1\n\nprompt",
            b"cairn-askpass/1\n\xff\nprompt",
        ] {
            assert_eq!(
                read_request(wire).unwrap(),
                Err(Malformed),
                "accepted {wire:?}"
            );
        }
    }

    #[test]
    fn an_answer_carries_the_secret_and_a_refusal_carries_nothing() {
        let value = format!("s-{}", std::process::id());
        let mut wire = Vec::new();
        write_answer(&mut wire, &Secret::from_string(value.clone())).unwrap();
        let secret = read_response(wire.as_slice()).unwrap().unwrap();
        assert_eq!(secret.expose_secret(), value.as_bytes());

        let mut wire = Vec::new();
        write_refusal(&mut wire).unwrap();
        assert!(read_response(wire.as_slice()).unwrap().is_none());
        assert!(read_response(b"".as_slice()).unwrap().is_none());
        assert!(read_response(b"answered".as_slice()).unwrap().is_none());
    }

    /// Content only: that the buffer is sized up front and never reallocated is
    /// not observable from safe code, and stands on `read_response`'s doc comment.
    #[test]
    fn a_kilobyte_answer_arrives_whole() {
        let mut wire = Vec::new();
        write_answer(&mut wire, &Secret::new(vec![9u8; 1024])).unwrap();
        let secret = read_response(wire.as_slice()).unwrap().unwrap();
        assert_eq!(secret.len(), 1024);
        assert!(secret.expose_secret().iter().all(|byte| *byte == 9));
    }
}
