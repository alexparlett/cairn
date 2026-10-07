//! A repository whose history is a line of commits written object by object with `std::fs` —
//! no `git` runs here (the guards scan this crate's tests, and none may build a `Command`), and
//! nothing is borrowed from this checkout, whose history is however a merge left it (#61).
//!
//! Each object is what git writes: `<kind> <length>\0<body>`, named by its SHA-1, stored as a
//! zlib stream under `objects/xx/…`. The streams use deflate's stored blocks, which every
//! inflater reads, so neither compression nor a hashing crate is needed: both are spelled out
//! below, small and checked against git's own empty tree id.

use cairn_model::Oid;

use super::fetch_tests::UnbornRepository;

/// A line of commits on `main`, `HEAD` on it.
pub(crate) struct WrittenRepository {
    fixture: UnbornRepository,
    /// The commits, newest first: the order a walk from `main` yields them.
    pub(crate) commits: Vec<Oid>,
}

/// Git's empty tree, every commit's tree here.
const EMPTY_TREE: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

impl WrittenRepository {
    /// `count` commits in a line, each a second newer than its parent, `main` at the newest.
    pub(crate) fn linear(name: &str, count: usize) -> Self {
        let fixture = UnbornRepository::new(&format!("{name}-{}", std::process::id()));
        let objects = fixture.path.join(".git/objects");
        let tree = write_object(&objects, "tree", b"");
        assert_eq!(tree, EMPTY_TREE, "the hashing is not git's");
        let mut parent: Option<String> = None;
        let mut oldest_first = Vec::with_capacity(count);
        for n in 0..count {
            let time = 1_600_000_000 + n;
            let mut body = format!("tree {EMPTY_TREE}\n");
            if let Some(parent) = &parent {
                body.push_str(&format!("parent {parent}\n"));
            }
            body.push_str(&format!(
                "author Ada <ada@example.com> {time} +0000\n\
                 committer Ada <ada@example.com> {time} +0000\n\ncommit {n}\n"
            ));
            let id = write_object(&objects, "commit", body.as_bytes());
            oldest_first.push(Oid::parse(&id).unwrap_or_else(|error| panic!("{id}: {error}")));
            parent = Some(id);
        }
        if let Some(newest) = &parent {
            let main = fixture.path.join(".git/refs/heads/main");
            std::fs::write(&main, format!("{newest}\n"))
                .unwrap_or_else(|error| panic!("writing {}: {error}", main.display()));
        }
        oldest_first.reverse();
        Self {
            fixture,
            commits: oldest_first,
        }
    }

    pub(crate) fn path(&self) -> &std::path::Path {
        &self.fixture.path
    }
}

/// Writes one loose object and answers its id in hex.
fn write_object(objects: &std::path::Path, kind: &str, body: &[u8]) -> String {
    let mut object = format!("{kind} {}\0", body.len()).into_bytes();
    object.extend_from_slice(body);
    let id: String = sha1(&object).iter().map(|b| format!("{b:02x}")).collect();
    let (dir, file) = id.split_at(2);
    let dir = objects.join(dir);
    std::fs::create_dir_all(&dir)
        .unwrap_or_else(|error| panic!("making {}: {error}", dir.display()));
    std::fs::write(dir.join(file), zlib_stored(&object))
        .unwrap_or_else(|error| panic!("writing {id}: {error}"));
    id
}

/// `data` as a zlib stream of stored (uncompressed) deflate blocks.
fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    let mut chunks = data.chunks(0xffff).peekable();
    if chunks.peek().is_none() {
        out.extend_from_slice(&[0x01, 0x00, 0x00, 0xff, 0xff]);
    }
    while let Some(chunk) = chunks.next() {
        out.push(u8::from(chunks.peek().is_none()));
        let len = u16::try_from(chunk.len()).unwrap_or(u16::MAX);
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&(!len).to_le_bytes());
        out.extend_from_slice(chunk);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for byte in data {
        a = (a + u32::from(*byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    out.extend_from_slice(&((b << 16) | a).to_be_bytes());
    out
}

/// SHA-1 (FIPS 180-4), for naming the objects written here.
fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [
        0x6745_2301,
        0xEFCD_AB89,
        0x98BA_DCFE,
        0x1032_5476,
        0xC3D2_E1F0,
    ];
    let mut message = data.to_vec();
    let bits = (data.len() as u64).wrapping_mul(8);
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bits.to_be_bytes());
    for block in message.chunks(64) {
        let mut w = [0u32; 80];
        for (i, word) in block.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = h;
        for (i, word) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5A82_7999),
                20..=39 => (b ^ c ^ d, 0x6ED9_EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC),
                _ => (b ^ c ^ d, 0xCA62_C1D6),
            };
            let next = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*word);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = next;
        }
        for (slot, value) in h.iter_mut().zip([a, b, c, d, e]) {
            *slot = slot.wrapping_add(value);
        }
    }
    let mut out = [0u8; 20];
    for (i, word) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The hashing and the stream are git's: SHA-1's published vectors, and a written line
    /// read back by the engine in order. Caught by: a hash or a stream git would not read.
    #[test]
    fn a_written_line_is_read_back_as_written() {
        let hex =
            |data: &[u8]| -> String { sha1(data).iter().map(|b| format!("{b:02x}")).collect() };
        assert_eq!(hex(b"abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(hex(b""), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        let repository = WrittenRepository::linear("cairn-written-line", 7);
        let repo = cairn_git::Repository::discover(repository.path())
            .unwrap_or_else(|error| panic!("opening: {error}"));
        let page = repo
            .history(
                &cairn_git::HistoryRequest::from_head(100),
                &cairn_git::CancelSignal::new(),
            )
            .unwrap_or_else(|error| panic!("walking: {error}"));
        assert_eq!(page.rows.ids().collect::<Vec<_>>(), repository.commits);
    }
}
