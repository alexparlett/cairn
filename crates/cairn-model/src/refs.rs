//! A repository's refs as one snapshot: its branches, remote-tracking refs and tags, its
//! stash list, and where `HEAD` is — what `git for-each-ref`, `git symbolic-ref`,
//! `git rev-parse` and `git stash list` answer, as plain data.

use crate::{Oid, RefName};

/// Which of the three namespaces a listed ref lives in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RefKind {
    /// Under `refs/heads/`.
    LocalBranch,
    /// Under `refs/remotes/`.
    RemoteTracking,
    /// Under `refs/tags/`.
    Tag,
}

/// What a ref names, as `%(objectname)` and `%(objecttype)` print it, with the commit it
/// identifies. For a symbolic ref this is what its target names: `origin/HEAD` names
/// whatever `origin/main` does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RefTarget {
    /// A commit, named directly.
    Commit(Oid),
    /// An annotated tag object. `object` is the tag the ref names — the outermost of a
    /// chain of tags — and `commit` the commit the chain ends at, or `None` when it ends at
    /// a tree or a blob.
    Tag { object: Oid, commit: Option<Oid> },
    /// A tree or a blob, named directly: such a ref is listed, and identifies no commit.
    Other(Oid),
}

impl RefTarget {
    /// The commit this identifies, if it identifies one.
    pub fn commit_id(&self) -> Option<Oid> {
        match self {
            Self::Commit(id) => Some(*id),
            Self::Tag { commit, .. } => *commit,
            Self::Other(_) => None,
        }
    }

    /// The object the ref names, before any tag is peeled: `%(objectname)`.
    pub fn object(&self) -> Oid {
        match self {
            Self::Commit(id) | Self::Other(id) => *id,
            Self::Tag { object, .. } => *object,
        }
    }
}

/// A local branch's upstream as git resolves it from `branch.<name>.remote` and
/// `branch.<name>.merge`: a remote-tracking ref, or — with `remote = .` — a local branch.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Upstream {
    /// The upstream ref exists; `commit` is the commit it identifies, `None` when it
    /// identifies none.
    Exists { name: RefName, commit: Option<Oid> },
    /// Configured, but no such ref exists: git's `[gone]`.
    Gone { name: RefName },
}

impl Upstream {
    /// The upstream's full name, as `%(upstream)` prints it.
    pub fn name(&self) -> &RefName {
        match self {
            Self::Exists { name, .. } | Self::Gone { name } => name,
        }
    }

    pub fn is_gone(&self) -> bool {
        match self {
            Self::Exists { .. } => false,
            Self::Gone { .. } => true,
        }
    }
}

/// One listed ref.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Ref {
    /// Its full name, `refs/heads/main`.
    pub name: RefName,
    pub kind: RefKind,
    pub target: RefTarget,
    /// For a symbolic ref, the ref at the end of its chain, as `%(symref)` prints it —
    /// `refs/remotes/origin/main` for `refs/remotes/origin/HEAD` — never peeled into it:
    /// the ref is listed as itself.
    pub symbolic: Option<RefName>,
    /// A local branch's upstream, when it has one configured; `None` for every other kind.
    pub upstream: Option<Upstream>,
}

impl Ref {
    /// The commit this ref identifies, if it identifies one.
    pub fn commit_id(&self) -> Option<Oid> {
        self.target.commit_id()
    }
}

/// Where `HEAD` is.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum HeadState {
    /// On a branch that has a commit: `git symbolic-ref HEAD` names it.
    Branch(RefName),
    /// Detached at the object `git rev-parse HEAD` prints.
    Detached(Oid),
    /// On a branch with no commit yet: `git symbolic-ref HEAD` names it, and
    /// `git rev-parse HEAD` fails.
    Unborn(RefName),
}

/// One entry of the stash list, `stash@{index}`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StashEntry {
    /// `0` is the newest.
    pub index: usize,
    /// The reflog's message, `On main: wip` or `WIP on main: 1234567 subject`.
    pub message: String,
    /// The stash commit itself.
    pub commit: Oid,
    /// The commit the stash was made on: the stash commit's first parent.
    pub base: Oid,
}

/// How far a local branch and its upstream have gone apart: what
/// `git rev-list --left-right --count <branch>...<upstream>` prints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct AheadBehind {
    /// Commits the branch has that its upstream lacks.
    pub ahead: usize,
    /// Commits the upstream has that the branch lacks.
    pub behind: usize,
}

/// Every local branch, remote-tracking ref and tag, the stash list, and `HEAD`, read at
/// one moment.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RefsSnapshot {
    /// In `git for-each-ref`'s order — bytewise by full name — which puts the local
    /// branches first, then the remote-tracking refs, then the tags.
    pub refs: Vec<Ref>,
    pub head: HeadState,
    /// Newest first, as `git stash list` prints it.
    pub stashes: Vec<StashEntry>,
    /// Refs skipped because they could not be read — content that is not a ref, an object
    /// that is not there, a symbolic chain that never ends — which git skips too.
    pub unreadable: usize,
}

impl RefsSnapshot {
    /// The refs of one kind, in order.
    pub fn of_kind(&self, kind: RefKind) -> impl Iterator<Item = &Ref> {
        self.refs
            .iter()
            .filter(move |reference| reference.kind == kind)
    }

    /// The ref with this full name, if it is listed. The list is sorted by name, so this
    /// is a binary search.
    pub fn find(&self, name: &RefName) -> Option<&Ref> {
        self.refs
            .binary_search_by(|reference| {
                reference
                    .name
                    .as_str()
                    .as_bytes()
                    .cmp(name.as_str().as_bytes())
            })
            .ok()
            .and_then(|at| self.refs.get(at))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn oid(byte: u8) -> Oid {
        Oid::from_bytes(&[byte; 20]).unwrap()
    }

    fn listed(name: &str, kind: RefKind, target: RefTarget) -> Ref {
        Ref {
            name: RefName::new(name),
            kind,
            target,
            symbolic: None,
            upstream: None,
        }
    }

    /// Caught by: a tag's commit read from its object, a tree taken for a commit, or a
    /// tag's object id lost to its peeled commit.
    #[test]
    fn each_target_names_its_object_and_identifies_its_commit() {
        assert_eq!(RefTarget::Commit(oid(1)).commit_id(), Some(oid(1)));
        assert_eq!(RefTarget::Commit(oid(1)).object(), oid(1));

        let annotated = RefTarget::Tag {
            object: oid(2),
            commit: Some(oid(3)),
        };
        assert_eq!(annotated.commit_id(), Some(oid(3)));
        assert_eq!(annotated.object(), oid(2), "the tag object was lost");

        let on_a_tree = RefTarget::Tag {
            object: oid(4),
            commit: None,
        };
        assert_eq!(on_a_tree.commit_id(), None);
        assert_eq!(on_a_tree.object(), oid(4));

        assert_eq!(RefTarget::Other(oid(5)).commit_id(), None);
        assert_eq!(RefTarget::Other(oid(5)).object(), oid(5));

        let reference = listed("refs/tags/v1", RefKind::Tag, annotated);
        assert_eq!(reference.commit_id(), Some(oid(3)));
    }

    /// Caught by: a gone upstream reported as present, or either variant losing its name.
    #[test]
    fn an_upstream_names_itself_whether_it_exists_or_is_gone() {
        let present = Upstream::Exists {
            name: RefName::new("refs/remotes/origin/main"),
            commit: Some(oid(1)),
        };
        let gone = Upstream::Gone {
            name: RefName::new("refs/remotes/origin/old"),
        };
        assert_eq!(present.name().as_str(), "refs/remotes/origin/main");
        assert!(!present.is_gone());
        assert_eq!(gone.name().as_str(), "refs/remotes/origin/old");
        assert!(gone.is_gone());
    }

    /// Caught by: `find` comparing anything but the bytes the list is sorted by (a
    /// shorthand, a locale order), or `of_kind` letting another kind through.
    #[test]
    fn a_snapshot_finds_a_ref_by_name_and_filters_by_kind() {
        let snapshot = RefsSnapshot {
            refs: vec![
                listed(
                    "refs/heads/a",
                    RefKind::LocalBranch,
                    RefTarget::Commit(oid(1)),
                ),
                listed(
                    "refs/heads/a-b",
                    RefKind::LocalBranch,
                    RefTarget::Commit(oid(2)),
                ),
                listed(
                    "refs/heads/a/b",
                    RefKind::LocalBranch,
                    RefTarget::Commit(oid(3)),
                ),
                listed(
                    "refs/remotes/origin/a",
                    RefKind::RemoteTracking,
                    RefTarget::Commit(oid(4)),
                ),
                listed("refs/tags/a", RefKind::Tag, RefTarget::Other(oid(5))),
            ],
            head: HeadState::Branch(RefName::new("refs/heads/a")),
            stashes: Vec::new(),
            unreadable: 0,
        };
        for reference in &snapshot.refs {
            assert_eq!(snapshot.find(&reference.name), Some(reference));
        }
        assert_eq!(snapshot.find(&RefName::new("refs/heads/b")), None);
        assert_eq!(
            snapshot.find(&RefName::new("a")),
            None,
            "found by shorthand"
        );
        let names = |kind| {
            snapshot
                .of_kind(kind)
                .map(|reference| reference.name.as_str())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            names(RefKind::LocalBranch),
            ["refs/heads/a", "refs/heads/a-b", "refs/heads/a/b"]
        );
        assert_eq!(names(RefKind::RemoteTracking), ["refs/remotes/origin/a"]);
        assert_eq!(names(RefKind::Tag), ["refs/tags/a"]);
    }

    /// Two snapshots are equal exactly when every part is: what a refresh compares to
    /// decide whether anything moved. Caught by: a field left out of the comparison.
    #[test]
    fn snapshots_differ_when_any_part_differs() {
        let base = RefsSnapshot {
            refs: vec![listed(
                "refs/heads/main",
                RefKind::LocalBranch,
                RefTarget::Commit(oid(1)),
            )],
            head: HeadState::Branch(RefName::new("refs/heads/main")),
            stashes: vec![StashEntry {
                index: 0,
                message: "On main: wip".to_owned(),
                commit: oid(7),
                base: oid(1),
            }],
            unreadable: 0,
        };
        assert_eq!(base.clone(), base);
        let mut moved = base.clone();
        moved.refs[0].target = RefTarget::Commit(oid(2));
        let mut retargeted = base.clone();
        retargeted.refs[0].symbolic = Some(RefName::new("refs/heads/other"));
        let mut detached = base.clone();
        detached.head = HeadState::Detached(oid(1));
        let mut stashed = base.clone();
        stashed.stashes.clear();
        let mut broken = base.clone();
        broken.unreadable = 1;
        let mut tracked = base.clone();
        tracked.refs[0].upstream = Some(Upstream::Gone {
            name: RefName::new("refs/remotes/origin/main"),
        });
        for changed in [moved, retargeted, detached, stashed, broken, tracked] {
            assert_ne!(changed, base);
        }
        assert_eq!(
            AheadBehind::default(),
            AheadBehind {
                ahead: 0,
                behind: 0
            }
        );
    }
}
