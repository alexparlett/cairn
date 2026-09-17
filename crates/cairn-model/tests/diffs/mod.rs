//! Fixture diffs, an independent oracle for what a selection means, and a seeded chooser.
//!
//! `unwrap` and `expect` are denied here: `clippy.toml`'s test exemption reaches code inside
//! a `#[test]` function, and these are helpers this crate's tests call.

use cairn_model::{
    ChangeStatus, ChangedFile, ChangedRange, DiffLine, FileMode, LineNumber, LineSpan, Oid,
    RepoPath, Selection, TextDiff, split_lines,
};

/// One fixture: a file, both its versions, and the diff between them.
pub struct Fixture {
    pub name: &'static str,
    pub file: ChangedFile,
    pub text: TextDiff,
}

impl Fixture {
    pub fn modified(name: &'static str, old: &[u8], new: &[u8]) -> Self {
        Self {
            name,
            file: file_at(
                "f.txt",
                ChangeStatus::Modified,
                Some(FileMode::Regular),
                Some(FileMode::Regular),
            ),
            text: text_of(old, new),
        }
    }

    pub fn added(name: &'static str, new: &[u8]) -> Self {
        Self {
            name,
            file: file_at("n.txt", ChangeStatus::Added, None, Some(FileMode::Regular)),
            text: text_of(b"", new),
        }
    }

    pub fn deleted(name: &'static str, old: &[u8]) -> Self {
        Self {
            name,
            file: file_at(
                "d.txt",
                ChangeStatus::Deleted,
                Some(FileMode::Regular),
                None,
            ),
            text: text_of(old, b""),
        }
    }

    pub fn renamed(name: &'static str, old: &[u8], new: &[u8], similarity: u8) -> Self {
        let mut file = file_at(
            "r.txt",
            ChangeStatus::Renamed(cairn_model::Similarity::from_percent(similarity)),
            Some(FileMode::Regular),
            Some(FileMode::Regular),
        );
        file.new_path = RepoPath::from("r2.txt");
        Self {
            name,
            file,
            text: text_of(old, new),
        }
    }

    pub fn mode_changed(name: &'static str, old: &[u8], new: &[u8]) -> Self {
        Self {
            name,
            file: file_at(
                "m.txt",
                ChangeStatus::Modified,
                Some(FileMode::Regular),
                Some(FileMode::Executable),
            ),
            text: text_of(old, new),
        }
    }

    /// The fixture with blob ids on both sides, which is what makes the emitter write an
    /// `index` line.
    pub fn with_ids(mut self) -> Self {
        self.file.old_id = Some(id_for(&self.text.old_content()));
        self.file.new_id = Some(id_for(&self.text.new_content()));
        self
    }
}

fn file_at(
    path: &str,
    status: ChangeStatus,
    old_mode: Option<FileMode>,
    new_mode: Option<FileMode>,
) -> ChangedFile {
    ChangedFile {
        status,
        old_path: RepoPath::from(path),
        new_path: RepoPath::from(path),
        old_mode,
        new_mode,
        old_id: None,
        new_id: None,
    }
}

/// A stand-in blob id: not git's hash, only something stable and different per content.
/// The emitter never computes an id, it only writes the one it is handed.
fn id_for(content: &[u8]) -> Oid {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in content {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    let hex = format!("{hash:016x}").repeat(3);
    let hex: String = hex.chars().take(40).collect();
    Oid::parse(&hex).unwrap_or_else(|e| panic!("the stand-in id {hex} is not an id: {e}"))
}

/// Builds a text diff from two versions, finding the changed ranges with a longest-common-
/// subsequence walk. Deliberately naive and test-only: the engine gets its ranges from the
/// diff library, and what matters here is only that the ranges are well formed.
pub fn text_of(old: &[u8], new: &[u8]) -> TextDiff {
    let old_lines = split_lines(old);
    let new_lines = split_lines(new);
    let changes = changed_ranges(&old_lines, &new_lines);
    TextDiff::new(old_lines, new_lines, changes)
}

fn changed_ranges(old: &[DiffLine], new: &[DiffLine]) -> Vec<ChangedRange> {
    let rows = old.len();
    let columns = new.len();
    // `common[i][j]`: the longest run of matching lines in `old[i..]` and `new[j..]`.
    let mut common = vec![vec![0usize; columns + 1]; rows + 1];
    for i in (0..rows).rev() {
        for j in (0..columns).rev() {
            let value = if old.get(i) == new.get(j) {
                common
                    .get(i + 1)
                    .and_then(|row| row.get(j + 1))
                    .copied()
                    .unwrap_or(0)
                    + 1
            } else {
                let down = common
                    .get(i + 1)
                    .and_then(|row| row.get(j))
                    .copied()
                    .unwrap_or(0);
                let across = common
                    .get(i)
                    .and_then(|row| row.get(j + 1))
                    .copied()
                    .unwrap_or(0);
                down.max(across)
            };
            if let Some(slot) = common.get_mut(i).and_then(|row| row.get_mut(j)) {
                *slot = value;
            }
        }
    }

    let mut changes = Vec::new();
    let (mut i, mut j) = (0usize, 0usize);
    while i < rows || j < columns {
        if i < rows && j < columns && old.get(i) == new.get(j) {
            i += 1;
            j += 1;
            continue;
        }
        let (start_old, start_new) = (i, j);
        while i < rows || j < columns {
            if i < rows && j < columns && old.get(i) == new.get(j) {
                break;
            }
            let down = common
                .get(i + 1)
                .and_then(|row| row.get(j))
                .copied()
                .unwrap_or(0);
            let across = common
                .get(i)
                .and_then(|row| row.get(j + 1))
                .copied()
                .unwrap_or(0);
            if j >= columns || (i < rows && down >= across) {
                i += 1;
            } else {
                j += 1;
            }
        }
        changes.push(ChangedRange::new(
            LineSpan::at(start_old as u32, (i - start_old) as u32),
            LineSpan::at(start_new as u32, (j - start_new) as u32),
        ));
    }
    changes
}

/// What a selection means, computed without hunks, headers, context or counts.
///
/// This is the oracle the round-trip is judged against, and it is deliberately the simplest
/// statement of the rule: an unselected removal stays, a selected removal goes, a selected
/// addition arrives where its change is, an unselected addition never happens. It shares no
/// code with the emitter or with the applier, so all three agreeing means something.
pub fn expected_result(text: &TextDiff, selection: &Selection) -> Vec<u8> {
    let mut bytes = Vec::new();
    for line in &expected_lines(text, selection) {
        bytes.extend_from_slice(line.bytes());
        if line.ends_with_newline() {
            bytes.push(b'\n');
        }
    }
    bytes
}

/// The same oracle, as lines, so a caller can ask whether the result is a well-formed file.
pub fn expected_lines(text: &TextDiff, selection: &Selection) -> Vec<DiffLine> {
    let mut out: Vec<DiffLine> = Vec::new();
    let mut cursor = 0u32;
    for change in text.changes() {
        for index in cursor..change.removed.start().index() {
            if let Some(line) = text.old_line(LineNumber::from_index(index)) {
                out.push(line.clone());
            }
        }
        for line in change.removed.numbers() {
            if !selection.holds_removed(line)
                && let Some(kept) = text.old_line(line)
            {
                out.push(kept.clone());
            }
        }
        for line in change.added.numbers() {
            if selection.holds_added(line)
                && let Some(arrived) = text.new_line(line)
            {
                out.push(arrived.clone());
            }
        }
        cursor = change.removed.end().index();
    }
    for index in cursor..(text.old_lines().len() as u32) {
        if let Some(line) = text.old_line(LineNumber::from_index(index)) {
            out.push(line.clone());
        }
    }
    out
}

/// Whether a result is a file a patch could be taken of again: only its last line may run
/// off the end. A selection that keeps an unterminated last line and adds lines after it
/// makes one that is not, which is what `git apply` makes of the same patch.
pub fn is_a_well_formed_file(lines: &[DiffLine]) -> bool {
    let last = lines.len().saturating_sub(1);
    lines
        .iter()
        .take(last)
        .all(cairn_model::DiffLine::ends_with_newline)
}

/// Every changed line of a diff, each named the way R1.3 names it.
pub fn every_identity(text: &TextDiff) -> Vec<(bool, u32)> {
    let mut identities = Vec::new();
    for change in text.changes() {
        identities.extend(change.removed.numbers().map(|line| (true, line.index())));
        identities.extend(change.added.numbers().map(|line| (false, line.index())));
    }
    identities
}

/// A selection that keeps each changed line with even odds, from a fixed seed.
pub fn seeded_selection(text: &TextDiff, seed: u64) -> Selection {
    let mut rng = Rng::new(seed);
    let mut selection = Selection::empty();
    for change in text.changes() {
        for line in change.removed.numbers() {
            if rng.coin() {
                selection.select_removed(line);
            }
        }
        for line in change.added.numbers() {
            if rng.coin() {
                selection.select_added(line);
            }
        }
    }
    selection
}

pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    fn next(&mut self) -> u64 {
        // xorshift64*, the same one the lane fixtures use.
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn coin(&mut self) -> bool {
        self.next().is_multiple_of(2)
    }
}

/// The fixtures the patch round-trip runs over: every awkward case the format allows.
pub fn corpus() -> Vec<Fixture> {
    vec![
        Fixture::modified(
            "one line in the middle",
            b"a\nb\nc\nd\ne\nf\ng\n",
            b"a\nb\nc\nD\ne\nf\ng\n",
        ),
        Fixture::modified(
            "a hunk at the first line",
            b"a\nb\nc\nd\ne\n",
            b"A\nb\nc\nd\ne\n",
        ),
        Fixture::modified(
            "a hunk at the last line",
            b"a\nb\nc\nd\ne\n",
            b"a\nb\nc\nd\nE\n",
        ),
        Fixture::modified("a one line file", b"solo\n", b"SOLO\n"),
        Fixture::modified("an empty file gaining content", b"", b"new\n"),
        Fixture::modified("a file losing all its content", b"gone\n", b""),
        Fixture::modified("an old side with no final newline", b"one", b"one\n"),
        Fixture::modified("a new side with no final newline", b"one\n", b"two"),
        Fixture::modified("neither side with a final newline", b"one", b"oneX"),
        Fixture::modified(
            "a last line edited where neither side ends",
            b"a\nb\nc\nlast",
            b"a\nb\nc\nLAST",
        ),
        Fixture::modified("CRLF content", b"a\r\nb\r\nc\r\n", b"a\r\nB\r\nc\r\n"),
        Fixture::modified(
            "CRLF content with no final newline",
            b"a\r\nb\r\nc\r",
            b"a\r\nB\r\nc\r",
        ),
        Fixture::modified(
            "blank lines around a change",
            b"a\n\n\nb\n\n\nc\n",
            b"a\n\n\nB\n\n\nc\n",
        ),
        Fixture::modified("a blank line added", b"a\nb\nc\n", b"a\n\nb\nc\n"),
        Fixture::modified("a blank line removed", b"a\n\nb\nc\n", b"a\nb\nc\n"),
        Fixture::modified(
            "adjacent hunks four lines apart",
            b"a\nb\nc\nd\ne\nf\ng\nh\ni\nj\nk\nl\n",
            b"A\nb\nc\nd\ne\nF\ng\nh\ni\nj\nk\nl\n",
        ),
        Fixture::modified(
            "hunks far enough apart to separate",
            b"a\nb\nc\nd\ne\nf\ng\nh\ni\nj\nk\nl\nm\nn\no\np\n",
            b"A\nb\nc\nd\ne\nf\ng\nh\ni\nj\nk\nl\nm\nn\no\nP\n",
        ),
        Fixture::modified(
            "an insertion and a removal in one file",
            b"a\nb\nc\nd\ne\nf\ng\nh\ni\nj\nk\nl\n",
            b"a\nX\nY\nb\nc\nd\ne\nf\ng\nh\nk\nl\n",
        ),
        Fixture::modified(
            "more lines added than removed",
            b"a\nb\nc\nd\n",
            b"a\nB\nC\nD\nE\nd\n",
        ),
        Fixture::modified(
            "more lines removed than added",
            b"a\nb\nc\nd\ne\nf\n",
            b"a\nB\nf\n",
        ),
        Fixture::added("a new file", b"alpha\nbeta\ngamma\n"),
        Fixture::added("a new file that never ends", b"alpha\nbeta"),
        Fixture::added("a new empty file", b""),
        Fixture::deleted("a deleted file", b"one\ntwo\nthree\n"),
        Fixture::deleted("a deleted empty file", b""),
        Fixture::renamed("a rename with no edit", b"same\n", b"same\n", 100),
        Fixture::renamed("a rename with edits", b"a\nb\nc\nd\n", b"a\nB\nc\nd\n", 75),
        Fixture::mode_changed("a mode change with no edit", b"same\n", b"same\n"),
        Fixture::mode_changed("a mode change with an edit", b"a\nb\n", b"a\nB\n"),
    ]
}
