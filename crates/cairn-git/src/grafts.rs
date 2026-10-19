//! The parents git substitutes for a commit's own: a shallow clone's boundary and the
//! repository's graft file.
//!
//! A boundary commit's object still names its parents, and gitoxide hands them over as the
//! object names them — the walk's `parent_ids` and `Commit::parent_ids` alike. git reads
//! the repository's `shallow` file and shows such a commit with NO parents: `git log
//! --format=%P` prints nothing for it, `git log --graph` draws it as a root, and `git
//! show` diffs it against the empty tree.
//!
//! git also reads `<common dir>/info/grafts`, one `commit parent...` line each, and shows
//! a grafted commit with the graft's parents in place of its own (`parse_commit_buffer` in
//! git's `commit.c`, at v2.30.9 and v2.56.0 alike; deprecated, but still read). gitoxide
//! reads no graft file. Every read that reports a commit's parents reads it through
//! `history::walk::Grafted`, which asks here, so Cairn shows what git shows.
//!
//! A line is read as git's `read_graft_line` reads it: trailing white space trimmed, an
//! empty line or one starting `#` skipped, then an id of the repository's hash and each
//! parent's after exactly one white-space character; a line that does not parse is
//! skipped, and of two lines for one commit the first is kept (`register_commit_graft`
//! with `ignore_dups`). A shallow commit is registered after the file and replaces a graft
//! for the same commit (`register_shallow`, in `shallow.c`). `GIT_GRAFT_FILE` in Cairn's own
//! environment is not honoured, as no other variable naming a repository's files is.

use std::collections::HashMap;

/// The repository's grafts as they were read once, for one query.
#[derive(Clone, Default)]
pub(crate) struct Grafts {
    /// The parents git shows for each grafted commit: none for a shallow commit.
    parents: HashMap<gix::ObjectId, Vec<gix::ObjectId>>,
    /// Whether the shallow file exists, which is what git asks (`is_repository_shallow`).
    shallow: bool,
}

/// What reading the grafts failed with.
#[derive(Debug, thiserror::Error)]
pub(crate) enum GraftsError {
    #[error(transparent)]
    Shallow(#[from] gix::shallow::read::Error),
    #[error("could not read {path}: {source}")]
    GraftFile {
        path: std::path::PathBuf,
        source: std::io::Error,
    },
}

impl Grafts {
    pub(crate) fn read(repo: &gix::Repository) -> Result<Self, GraftsError> {
        let path = repo.common_dir().join("info").join("grafts");
        let mut parents = match std::fs::read(&path) {
            Ok(bytes) => graft_file(&bytes, repo.object_hash()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => HashMap::new(),
            Err(source) => return Err(GraftsError::GraftFile { path, source }),
        };
        for commit in repo
            .shallow_commits()?
            .iter()
            .flat_map(|commits| commits.iter())
        {
            parents.insert(*commit, Vec::new());
        }
        Ok(Self {
            parents,
            shallow: repo.shallow_file().is_file(),
        })
    }

    /// Whether git reads no graft at all, shallow or from the file: one of the conditions
    /// under which git reads the commit-graph (`commit_graph_compatible`).
    pub(crate) fn is_empty(&self) -> bool {
        self.parents.is_empty() && !self.shallow
    }

    /// Each grafted commit and the parents git shows for it, in no order.
    pub(crate) fn grafted(&self) -> impl Iterator<Item = (&gix::ObjectId, &[gix::ObjectId])> {
        self.parents
            .iter()
            .map(|(commit, parents)| (commit, parents.as_slice()))
    }

    /// The parents git shows for `id` in place of its own, when it grafts `id`.
    pub(crate) fn of(&self, id: &gix::oid) -> Option<&[gix::ObjectId]> {
        if self.parents.is_empty() {
            return None;
        }
        self.parents.get(id).map(Vec::as_slice)
    }
}

/// Each graft a graft file's `bytes` hold, by commit.
fn graft_file(bytes: &[u8], hash: gix::hash::Kind) -> HashMap<gix::ObjectId, Vec<gix::ObjectId>> {
    let mut grafts = HashMap::new();
    for line in bytes.split(|byte| *byte == b'\n') {
        if let Some((commit, parents)) = graft_line(line, hash) {
            grafts.entry(commit).or_insert(parents);
        }
    }
    grafts
}

/// git's `isspace`, which is not C's: space, tab, line feed and carriage return
/// (`sane_ctype` in `git-compat-util.h`).
fn is_git_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r')
}

/// One graft line read as `read_graft_line` reads it, or `None` where git skips it.
fn graft_line(line: &[u8], hash: gix::hash::Kind) -> Option<(gix::ObjectId, Vec<gix::ObjectId>)> {
    let end = line
        .iter()
        .rposition(|byte| !is_git_space(*byte))
        .map_or(0, |last| last + 1);
    let line = line.get(..end)?;
    if line.is_empty() || line.first() == Some(&b'#') {
        return None;
    }
    let hex = hash.len_in_hex();
    let commit = gix::ObjectId::from_hex(line.get(..hex)?).ok()?;
    let mut rest = line.get(hex..)?;
    let mut parents = Vec::new();
    while let Some((&separator, after)) = rest.split_first() {
        if !is_git_space(separator) {
            return None;
        }
        parents.push(gix::ObjectId::from_hex(after.get(..hex)?).ok()?);
        rest = after.get(hex..)?;
    }
    Some((commit, parents))
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "1111111111111111111111111111111111111111";
    const B: &str = "2222222222222222222222222222222222222222";
    const C: &str = "3333333333333333333333333333333333333333";

    fn id(hex: &str) -> gix::ObjectId {
        gix::ObjectId::from_hex(hex.as_bytes()).unwrap()
    }

    /// Caught by: a comment or blank line read as a graft, trailing space or a carriage
    /// return refusing a line git takes, two separators or a short id accepted, or the
    /// second line for a commit replacing the first.
    #[test]
    fn a_graft_file_is_read_as_git_reads_it() {
        let file =
            format!("# a comment\n\n{A} {B} {C}  \r\n{B}\n{C}\t{A}\n{A} {C}\n{C}  {A}\n{B} 22\n");
        let grafts = graft_file(file.as_bytes(), gix::hash::Kind::Sha1);
        assert_eq!(
            grafts.get(&id(A)),
            Some(&vec![id(B), id(C)]),
            "the first line kept"
        );
        assert_eq!(
            grafts.get(&id(B)),
            Some(&Vec::new()),
            "a graft with no parents"
        );
        assert_eq!(grafts.get(&id(C)), Some(&vec![id(A)]), "a tab separates");
        assert_eq!(grafts.len(), 3, "{grafts:?}");
        assert_eq!(
            graft_line(format!("{C}  {A}").as_bytes(), gix::hash::Kind::Sha1),
            None
        );
        assert_eq!(
            graft_line(format!("{B} 22").as_bytes(), gix::hash::Kind::Sha1),
            None
        );
        assert_eq!(graft_line(b"  ", gix::hash::Kind::Sha1), None);
    }
}
