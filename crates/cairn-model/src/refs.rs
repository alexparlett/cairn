//! A repository's refs as one snapshot: its branches, remote-tracking refs and tags, its
//! stash list, and where `HEAD` is — what `git for-each-ref`, `git symbolic-ref`,
//! `git rev-parse` and `git stash list` answer, as plain data.

use crate::text_filter::{BETWEEN_CHECKS, Folded};
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

    /// Whether a history walked from `other` is the one walked from this: the same refs
    /// naming the same objects (a symbolic ref's target among them), the same `HEAD` and the
    /// same stash list (PRD R10.4). An upstream's configuration and the count of refs that
    /// could not be read draw nothing in the graph, so they are left out: a refresh that
    /// finds only those changed leaves the history alone.
    pub fn walks_as(&self, other: &Self) -> bool {
        self.head == other.head
            && self.stashes == other.stashes
            && self.refs.len() == other.refs.len()
            && self.refs.iter().zip(&other.refs).all(|(one, another)| {
                one.name == another.name
                    && one.kind == another.kind
                    && one.target == another.target
                    && one.symbolic == another.symbolic
            })
    }

    /// The refs and stashes whose names hold `text`, by index in [`Self::refs`] and
    /// [`Self::stashes`], in order — or `None` when `keep_going` says to stop, asked before
    /// the first and every few thousand after. The sidebar's filter (PRD R8.3): a ref by its
    /// name past its namespace (`main`, `origin/main`, `v1.0`), a stash by its message; the
    /// text anywhere in it, case ignored, as the Changes tab's file filter reads its text. An
    /// empty `text` matches everything. A pass over every ref, so a worker's.
    pub fn matching(
        &self,
        text: &str,
        mut keep_going: impl FnMut() -> bool,
    ) -> Option<RefsMatched> {
        let wanted = Folded::of(text);
        let mut matched = RefsMatched::default();
        for (index, listed) in self.refs.iter().enumerate() {
            if index % BETWEEN_CHECKS == 0 && !keep_going() {
                return None;
            }
            if wanted.found_in(listed.name.shorthand().as_bytes()) {
                matched.refs.push(u32::try_from(index).unwrap_or(u32::MAX));
            }
        }
        for (index, stash) in self.stashes.iter().enumerate() {
            if index % BETWEEN_CHECKS == 0 && !keep_going() {
                return None;
            }
            if wanted.found_in(stash.message.as_bytes()) {
                matched
                    .stashes
                    .push(u32::try_from(index).unwrap_or(u32::MAX));
            }
        }
        Some(matched)
    }
}

/// What [`RefsSnapshot::matching`] leaves: indices into the snapshot's refs and stashes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RefsMatched {
    pub refs: Vec<u32>,
    pub stashes: Vec<u32>,
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

    fn snapshot_of(names: &[&str], stashes: &[&str]) -> RefsSnapshot {
        RefsSnapshot {
            refs: names
                .iter()
                .enumerate()
                .map(|(n, name)| {
                    let kind = if name.starts_with("refs/heads/") {
                        RefKind::LocalBranch
                    } else if name.starts_with("refs/remotes/") {
                        RefKind::RemoteTracking
                    } else {
                        RefKind::Tag
                    };
                    listed(name, kind, RefTarget::Commit(oid(n as u8 + 1)))
                })
                .collect(),
            head: HeadState::Branch(RefName::new("refs/heads/main")),
            stashes: stashes
                .iter()
                .enumerate()
                .map(|(index, message)| StashEntry {
                    index,
                    message: (*message).to_owned(),
                    commit: oid(100 + index as u8),
                    base: oid(1),
                })
                .collect(),
            unreadable: 0,
        }
    }

    /// PRD R10.4: a refresh reopens the history when a ref moved, appeared or went, a
    /// symbolic ref was retargeted, `HEAD`'s state changed or the stash list did, and not
    /// for what the graph does not draw — an upstream's configuration, the count of refs
    /// that could not be read. Caught by: comparing whole snapshots (an upstream edit reopens
    /// the history), or leaving any drawn part out (a moved ref, a checkout that moves no
    /// ref, a dropped stash leave the graph stale).
    #[test]
    fn a_walk_is_the_same_unless_what_it_draws_changed() {
        let base = snapshot_of(&["refs/heads/main", "refs/heads/topic"], &["On main: wip"]);
        assert!(base.walks_as(&base.clone()));

        let mut tracked = base.clone();
        tracked.refs[0].upstream = Some(Upstream::Gone {
            name: RefName::new("refs/remotes/origin/main"),
        });
        let mut broken = base.clone();
        broken.unreadable = 3;
        for same in [tracked, broken] {
            assert_ne!(same, base);
            assert!(same.walks_as(&base), "{same:?}");
        }

        let mut moved = base.clone();
        moved.refs[1].target = RefTarget::Commit(oid(9));
        let mut retargeted = base.clone();
        retargeted.refs[0].symbolic = Some(RefName::new("refs/heads/topic"));
        let mut checked_out = base.clone();
        checked_out.head = HeadState::Branch(RefName::new("refs/heads/topic"));
        let mut stashed = base.clone();
        stashed.stashes[0].message = "On main: other".to_owned();
        let mut dropped = base.clone();
        dropped.stashes.clear();
        let mut gone = base.clone();
        gone.refs.pop();
        let mut renamed = base.clone();
        renamed.refs[1].name = RefName::new("refs/heads/topic2");
        let mut rekinded = base.clone();
        rekinded.refs[1].kind = RefKind::Tag;
        for changed in [
            moved,
            retargeted,
            checked_out,
            stashed,
            dropped,
            gone,
            renamed,
            rekinded,
        ] {
            assert!(!changed.walks_as(&base), "{changed:?}");
            assert!(!base.walks_as(&changed), "{changed:?}");
        }
    }

    /// PRD R8.3: the sidebar's filter keeps the refs whose name past its namespace holds the
    /// text, and the stashes whose message does, case ignored; an empty text keeps
    /// everything; a stop asked for is honoured. Caught by: matching the full name (`heads`
    /// would keep every branch), a case-sensitive match, stashes left out, or a filter that
    /// runs on after it was told to stop.
    #[test]
    fn the_sidebar_filter_keeps_the_names_that_hold_its_text() {
        let snapshot = snapshot_of(
            &[
                "refs/heads/Feature/Login",
                "refs/heads/main",
                "refs/remotes/origin/feature/login",
                "refs/tags/v1.0",
            ],
            &["On main: login wip", "WIP on main: other"],
        );
        assert_eq!(
            snapshot.matching("LOGIN", || true),
            Some(RefsMatched {
                refs: vec![0, 2],
                stashes: vec![0],
            })
        );
        assert_eq!(
            snapshot.matching("heads", || true),
            Some(RefsMatched::default()),
            "the namespace is not part of a name"
        );
        assert_eq!(
            snapshot.matching("origin/", || true),
            Some(RefsMatched {
                refs: vec![2],
                stashes: Vec::new(),
            })
        );
        assert_eq!(
            snapshot.matching("", || true),
            Some(RefsMatched {
                refs: vec![0, 1, 2, 3],
                stashes: vec![0, 1],
            })
        );
        assert_eq!(snapshot.matching("main", || false), None);
        let mut asked = 0;
        let stopped_at_stashes = snapshot.matching("main", || {
            asked += 1;
            asked < 2
        });
        assert_eq!(
            stopped_at_stashes, None,
            "the stashes' pass ran on after a stop"
        );
    }
}
