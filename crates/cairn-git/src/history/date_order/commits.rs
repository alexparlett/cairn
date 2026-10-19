//! The commits a date-order walk has read, each read once and kept for the life of the walk:
//! its parents as git shows them, its committer date and its generation, with the walk's
//! marks beside them.
//!
//! A commit is read from the commit-graph where the walk may use one (`walk::commit_graph`,
//! git's `commit_graph_compatible`) and the graph holds it, as git's `parse_commit_in_graph`
//! reads it, and from its object otherwise, through [`Grafted`], so a shallow boundary and a
//! graft read with the parents git shows. The map and each commit's record are
//! `gix_revwalk`'s (`IdMap`, `graph::Commit`); the walk reads into them itself rather than
//! through `gix_revwalk::Graph`, which borrows its commit-graph for its own lifetime, so a
//! walk that opens its own graph and is held across pages could not keep one.

use gix::objs::Find as _;
use gix::revwalk::graph::{Commit, IdMap};

use crate::Error;
use crate::history::walk::Grafted;

/// What the walk keeps of one commit: [`Mark`] beside what was read.
pub(super) type Node = Commit<Mark>;

/// git's per-commit walk state (`struct topo_walk_info`'s indegree slab, and the
/// `TOPO_WALK_INDEGREE` flag).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct Mark {
    /// 0 when the in-degree walk has not reached the commit, or the commit is emitted;
    /// otherwise 1 plus the children the in-degree walk reached that are not emitted yet.
    /// Ready to emit at 1.
    pub(super) indegree: u32,
    /// Whether the commit has entered the in-degree queue, which it does at most once.
    pub(super) queued: bool,
}

/// git's `GENERATION_NUMBER_INFINITY` as gix-commitgraph spells it: the generation of a
/// commit read without the commit-graph.
pub(super) const INFINITE: u32 = gix::commitgraph::GENERATION_NUMBER_INFINITY;

/// A commit's generation as the walk orders by it: [`INFINITE`] without one.
pub(super) fn generation(node: &Node) -> u32 {
    node.generation.unwrap_or(INFINITE)
}

/// A commit's committer date as git compares it: `timestamp_t` is unsigned, and a date
/// written negative reads through `strtoumax` as its two's complement (`parse_commit_date`
/// in git's `commit.c`).
pub(super) fn date(node: &Node) -> u64 {
    node.commit_time as u64
}

/// Every commit the walk has read, and where it reads them from.
pub(super) struct Commits<'repo> {
    objects: Grafted<'repo>,
    graph: Option<gix::commitgraph::Graph>,
    hash: gix::hash::Kind,
    nodes: IdMap<Node>,
    buffer: Vec<u8>,
    read: usize,
    from_graph: usize,
}

impl<'repo> Commits<'repo> {
    pub(super) fn new(
        objects: Grafted<'repo>,
        graph: Option<gix::commitgraph::Graph>,
        hash: gix::hash::Kind,
    ) -> Self {
        Self {
            objects,
            graph,
            hash,
            nodes: IdMap::default(),
            buffer: Vec::new(),
            read: 0,
            from_graph: 0,
        }
    }

    pub(super) fn get(&self, id: &gix::oid) -> Option<&Node> {
        self.nodes.get(id)
    }

    pub(super) fn get_mut(&mut self, id: &gix::oid) -> Option<&mut Node> {
        self.nodes.get_mut(id)
    }

    /// `id`, read once and kept; `None` when it is missing or is no commit, which a walk
    /// skips as git skips a commit it cannot parse.
    pub(super) fn read(&mut self, id: &gix::oid) -> Result<Option<&mut Node>, Error> {
        if !self.nodes.contains_key(id) {
            let Some(node) = self.load(id)? else {
                return Ok(None);
            };
            self.nodes.insert(id.to_owned(), node);
        }
        Ok(self.nodes.get_mut(id))
    }

    /// `id`'s committer date, read without keeping the commit: a stash's, whose own parents
    /// the walk never reads.
    pub(super) fn date_of(&mut self, id: &gix::oid) -> Result<Option<i64>, Error> {
        Ok(self.load(id)?.map(|node| node.commit_time))
    }

    /// Keeps `node` as `id`'s, in place of any read: a commit the walk is given rather than
    /// reads, such as a stash grafted onto its base.
    pub(super) fn insert(&mut self, id: gix::ObjectId, node: Node) {
        self.nodes.insert(id, node);
    }

    /// Commits read, from the graph or from their objects.
    pub(super) fn read_count(&self) -> usize {
        self.read
    }

    /// Commits read from the commit-graph.
    pub(super) fn read_from_graph(&self) -> usize {
        self.from_graph
    }

    /// The bytes the kept commits hold: the map at its capacity, a control byte per
    /// bucket, and the parents of a commit with more than one, which leave the record.
    pub(super) fn retained(&self) -> usize {
        let buckets = self.nodes.capacity();
        let spilled: usize = self
            .nodes
            .values()
            .filter(|node| node.parents.spilled())
            .map(|node| node.parents.capacity() * size_of::<gix::ObjectId>())
            .sum();
        buckets * (size_of::<(gix::ObjectId, Node)>() + 1) + spilled
    }

    fn load(&mut self, id: &gix::oid) -> Result<Option<Node>, Error> {
        let walk_error = |source: Box<dyn std::error::Error + Send + Sync>| Error::Walk { source };
        if let Some(graph) = &self.graph
            && let Some(position) = graph.lookup(id)
        {
            let commit = graph.commit_at(position);
            let mut parents = gix::traverse::commit::ParentIds::new();
            for parent in commit.iter_parents() {
                let parent = parent.map_err(|e| walk_error(Box::new(e)))?;
                parents.push(graph.id_at(parent).to_owned());
            }
            // A generation of zero is a graph written without generations, which git reads
            // as none (`commit_graph_generation`, v2.56.0 `commit-graph.c:126-135`).
            let generation = match commit.generation() {
                0 => None,
                generation => Some(generation),
            };
            self.read += 1;
            self.from_graph += 1;
            return Ok(Some(Node {
                parents,
                // CDAT's 34-bit date always fits.
                commit_time: i64::try_from(commit.committer_timestamp()).unwrap_or(i64::MAX),
                generation,
                data: Mark::default(),
            }));
        }
        let Some(object) = self
            .objects
            .try_find(id, &mut self.buffer)
            .map_err(walk_error)?
        else {
            return Ok(None);
        };
        if object.kind != gix::objs::Kind::Commit {
            return Ok(None);
        }
        let Some((parents, commit_time)) = header(object.data, self.hash.len_in_hex()) else {
            // A commit git cannot parse is skipped, as git skips it.
            return Ok(None);
        };
        self.read += 1;
        Ok(Some(Node {
            parents,
            // Kept as the field's type holds it; `date` reads it back unsigned.
            commit_time: commit_time as i64,
            generation: None,
            data: Mark::default(),
        }))
    }
}

/// A commit object's parents and committer date as git's `parse_commit_buffer` reads them
/// (`commit.c`, v2.30.9 and v2.56.0): a `tree` line, then each `parent` line, every other
/// header left unread; `None` where git refuses the commit. Reading only these lines is what
/// keeps the opening pass at git's own cost; a full parse reads both signatures.
fn header(data: &[u8], hex: usize) -> Option<(gix::traverse::commit::ParentIds, u64)> {
    let rest = data.strip_prefix(b"tree ")?;
    if rest.get(hex) != Some(&b'\n') {
        return None;
    }
    let mut rest = rest.get(hex + 1..)?;
    let mut parents = gix::traverse::commit::ParentIds::new();
    while let Some(line) = rest.strip_prefix(b"parent ") {
        if line.get(hex) != Some(&b'\n') {
            return None;
        }
        parents.push(gix::ObjectId::from_hex(line.get(..hex)?).ok()?);
        rest = line.get(hex + 1..)?;
    }
    Some((parents, commit_date(rest)))
}

/// git's `parse_commit_date` (v2.56.0 `commit.c:128`): the `author` line, then the
/// `committer` line, whose date is read after its last `>` by `strtoumax`, so a date written
/// negative reads as its two's complement and one that overflows as the largest; 0 where any
/// of it is missing. (v2.30.9 reads after the line's first `>`, which differs only for an
/// address holding a `>`.)
fn commit_date(rest: &[u8]) -> u64 {
    let Some(after_author) = rest
        .strip_prefix(b"author")
        .and_then(|line| line.iter().position(|byte| *byte == b'\n'))
        .and_then(|end| rest.get(b"author".len() + end + 1..))
    else {
        return 0;
    };
    if !after_author.starts_with(b"committer") {
        return 0;
    }
    let Some(line) = after_author
        .iter()
        .position(|byte| *byte == b'\n')
        .and_then(|end| after_author.get(..end))
    else {
        return 0;
    };
    let Some(after) = line
        .iter()
        .rposition(|byte| *byte == b'>')
        .and_then(|at| line.get(at + 1..))
    else {
        return 0;
    };
    let digits = after.trim_ascii_start();
    let (negative, digits) = match digits.first() {
        Some(b'-') => (true, digits.get(1..).unwrap_or_default()),
        Some(byte) if byte.is_ascii_digit() => (false, digits),
        _ => return 0,
    };
    let mut value: u64 = 0;
    for byte in digits.iter().take_while(|byte| byte.is_ascii_digit()) {
        match value
            .checked_mul(10)
            .and_then(|value| value.checked_add(u64::from(byte - b'0')))
        {
            Some(next) => value = next,
            None => return u64::MAX,
        }
    }
    if negative {
        value.wrapping_neg()
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TREE: &str = "tree 4b825dc642cb6eb9a060e54bf8d69288fbee4904\n";
    const P1: &str = "1111111111111111111111111111111111111111";
    const P2: &str = "2222222222222222222222222222222222222222";

    fn commit(parents: &[&str], author: &str, committer: &str) -> Vec<u8> {
        let mut text = TREE.to_owned();
        for parent in parents {
            text.push_str(&format!("parent {parent}\n"));
        }
        text.push_str(author);
        text.push_str(committer);
        text.push_str("gpgsig -----BEGIN-----\n parent 3333\n\nmessage\n");
        text.into_bytes()
    }

    /// Caught by: parents read past the header or out of order, a date read from the
    /// author, a negative date not read as git's unsigned one, or a commit git refuses read.
    #[test]
    fn a_commit_header_is_read_as_git_reads_it() {
        let read = |bytes: &[u8]| {
            header(bytes, 40).map(|(parents, date)| {
                let parents: Vec<String> = parents.iter().map(|id| id.to_string()).collect();
                (parents, date)
            })
        };
        let author = "author A <a@example.com> 100 +0000\n";
        let committer = "committer C <c@example.com> 200 +0100\n";
        assert_eq!(
            read(&commit(&[P2, P1], author, committer)),
            Some((vec![P2.to_owned(), P1.to_owned()], 200))
        );
        assert_eq!(read(&commit(&[], author, committer)), Some((vec![], 200)));
        let negative = "committer C <c@example.com> -5 +0000\n";
        assert_eq!(
            read(&commit(&[P1], author, negative)).map(|(_, date)| date),
            Some((-5i64) as u64)
        );
        let huge = "committer C <c@example.com> 99999999999999999999999 +0000\n";
        assert_eq!(
            read(&commit(&[], author, huge)).map(|(_, date)| date),
            Some(u64::MAX)
        );
        let angled = "committer C <c>d> 300 +0000\n";
        assert_eq!(
            read(&commit(&[], author, angled)).map(|(_, date)| date),
            Some(300)
        );
        let no_author = commit(&[], "", committer);
        assert_eq!(read(&no_author).map(|(_, date)| date), Some(0));
        let no_date = "committer C <c@example.com> +0000\n";
        assert_eq!(
            read(&commit(&[], author, no_date)).map(|(_, date)| date),
            Some(0)
        );
        assert_eq!(read(b"tree 4b825dc6\n"), None, "a short tree id");
        assert_eq!(
            read(format!("{TREE}parent 1111\n").as_bytes()),
            None,
            "a short parent id"
        );
    }
}
