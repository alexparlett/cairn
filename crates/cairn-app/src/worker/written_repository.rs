//! A repository whose history is a line of commits written object by object with `std::fs` —
//! no `git` runs here (the guards scan this crate's tests, and none may build a `Command`), and
//! nothing is borrowed from this checkout, whose history is however a merge left it (#61).
//!
//! Each object is what git writes: `<kind> <length>\0<body>`, named by its SHA-1, stored as a
//! zlib stream under `objects/xx/…`. The streams use deflate's stored blocks, which every
//! inflater reads, so neither compression nor a hashing crate is needed: both are spelled out
//! below, small and checked against git's own empty tree id. A long line is written as one
//! pack and its version 2 index instead (`gitformat-pack`), so a history of a few hundred
//! thousand commits costs two files rather than a file a commit.

use cairn_model::Oid;

use super::network::tests::UnbornRepository;

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
            let body = commit_body(n, parent.as_deref());
            let id = write_object(&objects, "commit", body.as_bytes());
            oldest_first.push(Oid::parse(&id).unwrap_or_else(|error| panic!("{id}: {error}")));
            parent = Some(id);
        }
        Self::pointed(fixture, oldest_first)
    }

    /// [`Self::linear`] written as one pack and its index rather than as loose objects: a
    /// history long enough that opening a walk over it takes a measurable while.
    pub(crate) fn packed(name: &str, count: usize) -> Self {
        let fixture = UnbornRepository::new(&format!("{name}-{}", std::process::id()));
        let mut objects: Vec<(u8, Vec<u8>)> = Vec::with_capacity(count + 1);
        // Pack object types: 1 a commit, 2 a tree.
        objects.push((2, Vec::new()));
        let mut parent: Option<String> = None;
        let mut oldest_first = Vec::with_capacity(count);
        for n in 0..count {
            let body = commit_body(n, parent.as_deref()).into_bytes();
            let id = hex(&sha1(&loose_form("commit", &body)));
            oldest_first.push(Oid::parse(&id).unwrap_or_else(|error| panic!("{id}: {error}")));
            parent = Some(id);
            objects.push((1, body));
        }
        let pack_dir = fixture.path.join(".git/objects/pack");
        std::fs::create_dir_all(&pack_dir)
            .unwrap_or_else(|error| panic!("making {}: {error}", pack_dir.display()));
        let (pack, index, name) = pack_of(&objects);
        for (extension, bytes) in [("pack", pack), ("idx", index)] {
            let path = pack_dir.join(format!("pack-{name}.{extension}"));
            std::fs::write(&path, bytes)
                .unwrap_or_else(|error| panic!("writing {}: {error}", path.display()));
        }
        Self::pointed(fixture, oldest_first)
    }

    /// `main` at the newest of `oldest_first`.
    fn pointed(fixture: UnbornRepository, mut oldest_first: Vec<Oid>) -> Self {
        if let Some(newest) = oldest_first.last() {
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

/// The `n`th commit of a line, a second newer than its parent.
fn commit_body(n: usize, parent: Option<&str>) -> String {
    let time = 1_600_000_000 + n;
    let mut body = format!("tree {EMPTY_TREE}\n");
    if let Some(parent) = parent {
        body.push_str(&format!("parent {parent}\n"));
    }
    body.push_str(&format!(
        "author Ada <ada@example.com> {time} +0000\n\
         committer Ada <ada@example.com> {time} +0000\n\ncommit {n}\n"
    ));
    body
}

/// An object as it is named and stored loose: `<kind> <length>\0<body>`.
fn loose_form(kind: &str, body: &[u8]) -> Vec<u8> {
    let mut object = format!("{kind} {}\0", body.len()).into_bytes();
    object.extend_from_slice(body);
    object
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A version 2 pack of `objects` — each its pack type (1 a commit, 2 a tree) and body —
/// its version 2 index, and the pack's checksum in hex, which names both files.
fn pack_of(objects: &[(u8, Vec<u8>)]) -> (Vec<u8>, Vec<u8>, String) {
    let mut pack = b"PACK".to_vec();
    pack.extend_from_slice(&2u32.to_be_bytes());
    let count = u32::try_from(objects.len()).unwrap_or_else(|_| panic!("too many objects"));
    pack.extend_from_slice(&count.to_be_bytes());
    // Each object's id, its offset in the pack and the CRC-32 of its packed bytes.
    let mut entries: Vec<([u8; 20], u32, u32)> = Vec::with_capacity(objects.len());
    for (kind, body) in objects {
        let name = if *kind == 1 { "commit" } else { "tree" };
        let id = sha1(&loose_form(name, body));
        let offset = u32::try_from(pack.len()).unwrap_or_else(|_| panic!("a pack past 2 GiB"));
        let start = pack.len();
        // The type and size: four bits of size in the first byte, seven in each after.
        let mut size = body.len();
        let mut byte = (kind << 4) | u8::try_from(size & 0x0f).unwrap_or(0);
        size >>= 4;
        while size > 0 {
            pack.push(byte | 0x80);
            byte = u8::try_from(size & 0x7f).unwrap_or(0);
            size >>= 7;
        }
        pack.push(byte);
        pack.extend_from_slice(&zlib_stored(body));
        entries.push((id, offset, crc32(&pack[start..])));
    }
    let checksum = sha1(&pack);
    pack.extend_from_slice(&checksum);

    entries.sort_by_key(|(id, ..)| *id);
    let mut index = vec![0xff, b't', b'O', b'c'];
    index.extend_from_slice(&2u32.to_be_bytes());
    let mut fanout = [0u32; 256];
    for (id, ..) in &entries {
        fanout[usize::from(id[0])] += 1;
    }
    let mut running = 0u32;
    for slot in &mut fanout {
        running += *slot;
        index.extend_from_slice(&running.to_be_bytes());
    }
    for (id, ..) in &entries {
        index.extend_from_slice(id);
    }
    for (_, _, crc) in &entries {
        index.extend_from_slice(&crc.to_be_bytes());
    }
    for (_, offset, _) in &entries {
        index.extend_from_slice(&offset.to_be_bytes());
    }
    index.extend_from_slice(&checksum);
    let own = sha1(&index);
    index.extend_from_slice(&own);
    (pack, index, hex(&checksum))
}

/// CRC-32 (IEEE 802.3, reflected), as a pack index records one per object.
fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// Writes one loose object and answers its id in hex.
fn write_object(objects: &std::path::Path, kind: &str, body: &[u8]) -> String {
    let object = loose_form(kind, body);
    let id = hex(&sha1(&object));
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

    /// A packed line is read back as written, and its checksums are what readers check:
    /// CRC-32's published check value, and the walk reading every commit of the pack in order.
    /// Caught by: a pack or an index a reader refuses, or an object named otherwise than its
    /// loose form would be.
    #[test]
    fn a_packed_line_is_read_back_as_written() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        let repository = WrittenRepository::packed("cairn-packed-line", 300);
        let repo = cairn_git::Repository::discover(repository.path())
            .unwrap_or_else(|error| panic!("opening: {error}"));
        let page = repo
            .history(
                &cairn_git::HistoryRequest::from_head(1_000),
                &cairn_git::CancelSignal::new(),
            )
            .unwrap_or_else(|error| panic!("walking: {error}"));
        assert_eq!(page.rows.ids().collect::<Vec<_>>(), repository.commits);
    }
}
