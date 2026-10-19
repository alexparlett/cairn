//! What a walk from a refs snapshot starts from, and what its rows carry from it: every
//! commit a local branch, a remote-tracking ref or a tag identifies, and `HEAD`'s — Fork's
//! All Commits, git's `rev-list --branches --remotes --tags HEAD` — with the refs that label
//! each, and the stash list. Neither `refs/stash` nor the commit a stash was made on seeds
//! the refs' walk; in git's date order each stash is a tip of its own whose one parent is its
//! base (`date_order`). A tag naming a tree or a blob identifies no commit, so it is no seed
//! and labels no row.
//!
//! [`RefSeeds`] is read from the snapshot alone, as plain data; [`Decoration`] is what a
//! walk resolves it to once it opens — each stash's commit read for what its row draws —
//! and shares, unchanged, with every page and cursor of that walk.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use cairn_model::{HeadState, Label, Oid, RefKind, RefName, RefsSnapshot, StashEntry};

use crate::Error;
use crate::commit_encoding::CommitEncoding;
use crate::object_id::object_id;

use super::walk::Grafted;
use gix::objs::FindHeader as _;

/// One ref labelling a commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SeedLabel {
    name: RefName,
    kind: RefKind,
    current: bool,
}

impl SeedLabel {
    fn label(&self) -> Label<'_> {
        Label {
            name: self.name.as_str(),
            kind: self.kind,
            current: self.current,
        }
    }
}

/// A walk's seeds, labels and stashes, read from one snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RefSeeds {
    /// Every commit a listed ref identifies, each once, in the snapshot's order.
    tips: Vec<Oid>,
    /// `HEAD`'s commit: its branch's, or the one it is detached at.
    head: Option<Oid>,
    /// Whether `head` was read from a detached `HEAD`, which may name what is not a commit.
    detached: bool,
    /// Shared with the walk's [`Decoration`]: one entry per labelled commit, so never copied.
    labels: Arc<HashMap<Oid, Vec<SeedLabel>>>,
    stashes: Vec<StashEntry>,
    /// Every local branch the snapshot lists, by full name: whose reflogs Show Lost Commits
    /// reads, beside `HEAD`'s.
    branches: Arc<[RefName]>,
}

impl RefSeeds {
    pub(super) fn of(snapshot: &RefsSnapshot) -> Self {
        let current = match &snapshot.head {
            HeadState::Branch(name) => Some(name),
            HeadState::Detached(_) | HeadState::Unborn(_) => None,
        };
        let mut tips = Vec::new();
        let mut seen = HashSet::new();
        let mut labels: HashMap<Oid, Vec<SeedLabel>> = HashMap::new();
        let branches = snapshot
            .refs
            .iter()
            .filter(|listed| listed.kind == RefKind::LocalBranch)
            .map(|listed| listed.name.clone())
            .collect();
        for listed in &snapshot.refs {
            let Some(commit) = listed.commit_id() else {
                // A tag on a tree or a blob: no seed, and no row to label.
                continue;
            };
            if seen.insert(commit) {
                tips.push(commit);
            }
            labels.entry(commit).or_default().push(SeedLabel {
                name: listed.name.clone(),
                kind: listed.kind,
                current: current == Some(&listed.name),
            });
        }
        let (head, detached) = match &snapshot.head {
            HeadState::Branch(name) => (
                snapshot.find(name).and_then(|branch| branch.commit_id()),
                false,
            ),
            HeadState::Detached(id) => (Some(*id), true),
            HeadState::Unborn(_) => (None, false),
        };
        Self {
            tips,
            head,
            detached,
            labels: Arc::new(labels),
            stashes: snapshot.stashes.clone(),
            branches,
        }
    }

    /// Every local branch the snapshot lists, by full name.
    pub(super) fn branches(&self) -> &Arc<[RefName]> {
        &self.branches
    }
}

/// A stash as the walk places and draws its row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Stash {
    pub(super) index: usize,
    pub(super) id: Oid,
    pub(super) base: Oid,
    pub(super) message: String,
    pub(super) author: String,
    pub(super) author_time: i64,
    /// The stash commit's committer date: which of the stashes on one base goes first, in an
    /// order whose walk yields no stash (`stream`).
    pub(super) committed: i64,
}

/// What a walk's rows carry beyond their commits, shared by every page and cursor of it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Decoration {
    labels: Arc<HashMap<Oid, Vec<SeedLabel>>>,
    head: Option<Oid>,
    stashes: Vec<Stash>,
}

impl Decoration {
    /// The refs labelling `commit`, in the snapshot's order, as the page takes them.
    pub(super) fn labels_of(&self, commit: &Oid) -> Vec<Label<'_>> {
        self.labels
            .get(commit)
            .map(|labels| labels.iter().map(SeedLabel::label).collect())
            .unwrap_or_default()
    }

    pub(super) fn is_head(&self, commit: &Oid) -> bool {
        self.head.as_ref() == Some(commit)
    }

    pub(super) fn stashes(&self) -> &[Stash] {
        &self.stashes
    }
}

/// `seeds` resolved against the repository: the tips the walk starts from, and its
/// decoration. A detached `HEAD` naming what is not a commit seeds nothing, as it labels
/// nothing. A stash whose commit cannot be read — dropped and pruned since the snapshot —
/// has no row, as the snapshot's own stash read skips an entry it cannot read; the walk
/// goes on without it. A stash commit filed more than once is one row, its newest entry's.
/// Reads one commit per stash and, for a detached `HEAD`, one header;
/// the labels are shared, not copied.
pub(super) fn resolve(
    repo: &gix::Repository,
    seeds: &RefSeeds,
) -> Result<(Vec<Oid>, Decoration), Error> {
    // Read as git reads them, through a replace ref and a graft: a detached `HEAD`'s object
    // and each stash's commit.
    let objects = Grafted::read(repo)?;
    let mut tips = seeds.tips.clone();
    let mut head = seeds.head;
    // Every tip labels its commit, so a commit with no label is no tip yet.
    if let Some(id) = seeds.head
        && !seeds.labels.contains_key(&id)
    {
        if seeds.detached && !is_commit(&objects, &id)? {
            head = None;
        } else {
            tips.push(id);
        }
    }
    // One row per stash commit: `git stash store` can file one commit twice, and a commit is
    // one row in a graph — the newest entry's, the list being newest first. The sidebar lists
    // every entry from the snapshot.
    let mut seen = HashSet::new();
    let stashes = seeds
        .stashes
        .iter()
        .filter(|entry| seen.insert(entry.commit))
        .filter_map(|entry| stash_of(&objects, entry).ok())
        .collect();
    Ok((
        tips,
        Decoration {
            labels: Arc::clone(&seeds.labels),
            head,
            stashes,
        },
    ))
}

/// Whether `id` is a commit as git reads it. A replacement git would die on is taken as one, so
/// the walk reads it and fails as git does rather than leave `HEAD` out.
fn is_commit(objects: &Grafted<'_>, id: &Oid) -> Result<bool, Error> {
    Ok(match objects.try_header(&object_id(id)?) {
        Ok(Some(header)) => header.kind == gix::object::Kind::Commit,
        Ok(None) => false,
        Err(_) => true,
    })
}

/// The stash `entry` names, its commit read for its author and its dates.
fn stash_of(objects: &Grafted<'_>, entry: &StashEntry) -> Result<Stash, Error> {
    let id = entry.commit;
    let read = |source: Box<dyn std::error::Error + Send + Sync>| Error::ReadCommit {
        id: id.to_string(),
        source,
    };
    let mut buffer = Vec::new();
    let (decoded, bytes) = crate::commit::read_commit(objects, &id, &mut buffer)?;
    let encoding = CommitEncoding::of_decoded(&decoded, bytes);
    let author = decoded.author().map_err(|e| read(Box::new(e)))?;
    let author_time = author.time().map_err(|e| read(Box::new(e)))?.seconds;
    let committed = decoded
        .committer()
        .map_err(|e| read(Box::new(e)))?
        .time()
        .map_err(|e| read(Box::new(e)))?
        .seconds;
    Ok(Stash {
        index: entry.index,
        id,
        base: entry.base,
        message: entry.message.clone(),
        author: encoding.text(author.name),
        author_time,
        committed,
    })
}
