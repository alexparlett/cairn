//! The sidebar's list of refs as rows (refs-and-status R8.1-R8.4): its sections in Fork's
//! order — Branches, Remotes, Tags, Stashes — branches and remote-tracking refs grouped into
//! folders split at `/`, each a row of its own, and only what its open sections and folders
//! show. Laid out from a snapshot, a filter's text and what is open, in one pass over every
//! ref, so a worker's — the window keeps the rows it answers and reads each ref's name from
//! the snapshot by index as it draws the row.
//!
//! At each level of a section, folders come first and then the refs, each in natural order —
//! case ignored, a run of digits read as a number, so `b2` comes before `b10` and `Alpha`
//! before `beta` before `Gamma` ([`natural_order`]): Fork's natural sort, folders first (the
//! user's decision, 2026-10-07). Tags are listed whole in the same order; stashes in the stash
//! list's. The snapshot's own order (bytewise by name) sorts every name under one folder
//! together — the names between two that start `feature/` start `feature/` too — so a folder
//! is found as a run of the refs at its level, and only then put in its place.
//!
//! While the filter holds text every section and folder is drawn open, so what matches is
//! never hidden behind a closed one, and a section with no match has no caption (the user's
//! decision, 2026-10-07). A detached `HEAD` is the first row under Branches, as Fork draws
//! it.

use std::cmp::Ordering;
use std::collections::BTreeSet;

use crate::refs::{HeadState, RefKind, RefsSnapshot};
use crate::text_filter::{BETWEEN_CHECKS, Folded};

/// A section of the sidebar's refs, in Fork's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SidebarSection {
    Branches,
    Remotes,
    Tags,
    Stashes,
}

impl SidebarSection {
    /// Every section, in the order the sidebar draws them.
    pub const ALL: [Self; 4] = [Self::Branches, Self::Remotes, Self::Tags, Self::Stashes];

    /// The namespace whose refs the section lists, `None` for the stashes.
    fn kind(self) -> Option<RefKind> {
        match self {
            Self::Branches => Some(RefKind::LocalBranch),
            Self::Remotes => Some(RefKind::RemoteTracking),
            Self::Tags => Some(RefKind::Tag),
            Self::Stashes => None,
        }
    }

    /// Whether the section's refs are grouped into folders by `/` (R8.2): branches and
    /// remote-tracking refs, never tags.
    fn has_folders(self) -> bool {
        match self {
            Self::Branches | Self::Remotes => true,
            Self::Tags | Self::Stashes => false,
        }
    }
}

/// Which sections are closed and which folders are open: what the sidebar's rows are laid
/// out under. A section is open until it is closed; a folder is closed until it is opened.
/// A folder is named by its full path, `refs/heads/feature` or `refs/remotes/origin`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Disclosure {
    closed_sections: BTreeSet<SidebarSection>,
    open_folders: BTreeSet<String>,
}

impl Disclosure {
    pub fn is_section_open(&self, section: SidebarSection) -> bool {
        !self.closed_sections.contains(&section)
    }

    pub fn is_folder_open(&self, path: &str) -> bool {
        self.open_folders.contains(path)
    }

    /// Opens a closed section and closes an open one.
    pub fn toggle_section(&mut self, section: SidebarSection) {
        if !self.closed_sections.remove(&section) {
            self.closed_sections.insert(section);
        }
    }

    /// Opens a closed folder and closes an open one.
    pub fn toggle_folder(&mut self, path: &str) {
        if !self.open_folders.remove(path) {
            self.open_folders.insert(path.to_owned());
        }
    }

    /// Opens every folder `name` sits in — how the current branch is revealed — and says
    /// whether any was closed.
    pub fn reveal(&mut self, name: &str) -> bool {
        let mut opened = false;
        for (at, _) in name.match_indices('/') {
            let Some(path) = name.get(..at) else {
                continue;
            };
            if folder_depth(path).is_some() && !self.open_folders.contains(path) {
                self.open_folders.insert(path.to_owned());
                opened = true;
            }
        }
        opened
    }
}

/// How deep a folder path is under its namespace — `refs/heads/a` is 0, `refs/heads/a/b` 1 —
/// or `None` for a path that names no folder (a namespace itself, or none of the two that
/// have folders).
fn folder_depth(path: &str) -> Option<usize> {
    ["refs/heads/", "refs/remotes/"]
        .iter()
        .find_map(|namespace| path.strip_prefix(namespace))
        .filter(|rest| !rest.is_empty())
        .map(|rest| rest.matches('/').count())
}

/// One row of the sidebar's refs. `Copy` and twelve bytes: a list of tens of thousands of
/// them costs no allocation of its own. A ref, a folder and a stash name their place in the
/// snapshot the rows were laid out from, which the window keeps beside them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SidebarRow {
    /// A section's caption.
    Section { section: SidebarSection, open: bool },
    /// A folder `depth` folders deep in its section: its name is the `depth`th part of the
    /// name of the ref at `first`, the first ref under it ([`folder_name`], [`folder_path`]).
    Folder { first: u32, depth: u16, open: bool },
    /// The ref at `index` in the snapshot, under `depth` folders.
    Ref { index: u32, depth: u16 },
    /// `HEAD`, detached: the first row under Branches.
    DetachedHead,
    /// The stash at `index` in the snapshot's stash list.
    Stash { index: u32 },
}

/// The name a folder row draws: the `depth`th part of its first ref's name past the
/// namespace — `feature` for `refs/heads/feature/login` at depth 0, `origin` for
/// `refs/remotes/origin/main`.
pub fn folder_name(snapshot: &RefsSnapshot, first: u32, depth: u16) -> &str {
    snapshot
        .refs
        .get(first as usize)
        .and_then(|listed| listed.name.shorthand().split('/').nth(usize::from(depth)))
        .unwrap_or("")
}

/// The full path a folder is opened and closed by: its first ref's name up to and with the
/// folder's own part — `refs/heads/feature` for `refs/heads/feature/login` at depth 0.
pub fn folder_path(snapshot: &RefsSnapshot, first: u32, depth: u16) -> &str {
    let Some(name) = snapshot
        .refs
        .get(first as usize)
        .map(|listed| listed.name.as_str())
    else {
        return "";
    };
    let namespace = name.len() - short_len(name);
    let end = name
        .match_indices('/')
        .filter(|(at, _)| *at >= namespace)
        .nth(usize::from(depth))
        .map_or(name.len(), |(at, _)| at);
    name.get(..end).unwrap_or(name)
}

/// The length of `name` past its namespace.
fn short_len(name: &str) -> usize {
    crate::RefName::new(name).shorthand().len()
}

/// Fork's natural order of two names: case ignored, each run of ASCII digits compared as the
/// number it spells (`b2` before `b10`), everything else character by character as Unicode
/// lowercases it. Names that differ only in case or in a number's leading zeros are put in
/// bytewise order, so the order is total.
pub fn natural_order(one: &str, other: &str) -> Ordering {
    let (mut a, mut b) = (one, other);
    loop {
        match (a.chars().next(), b.chars().next()) {
            (None, None) => return one.cmp(other),
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let (digits_a, rest_a) = split_digits(a);
                let (digits_b, rest_b) = split_digits(b);
                let (value_a, value_b) = (
                    digits_a.trim_start_matches('0'),
                    digits_b.trim_start_matches('0'),
                );
                let by_value = value_a
                    .len()
                    .cmp(&value_b.len())
                    .then_with(|| value_a.cmp(value_b));
                if by_value != Ordering::Equal {
                    return by_value;
                }
                (a, b) = (rest_a, rest_b);
            }
            (Some(x), Some(y)) => {
                let by_letter = x.to_lowercase().cmp(y.to_lowercase());
                if by_letter != Ordering::Equal {
                    return by_letter;
                }
                (a, b) = (&a[x.len_utf8()..], &b[y.len_utf8()..]);
            }
        }
    }
}

/// `name`'s leading run of ASCII digits, and the rest.
fn split_digits(name: &str) -> (&str, &str) {
    let end = name
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(name.len());
    name.split_at(end)
}

/// How many entries are laid out between two asks of `keep_going`.
struct Pass<F> {
    keep_going: F,
    seen: usize,
}

impl<F: FnMut() -> bool> Pass<F> {
    /// One more entry; `false` once `keep_going` says to stop, asked before the first and
    /// every few thousand after.
    fn tick(&mut self) -> bool {
        let ask = self.seen.is_multiple_of(BETWEEN_CHECKS);
        self.seen += 1;
        !ask || (self.keep_going)()
    }
}

impl RefsSnapshot {
    /// The sidebar's rows for this snapshot (R8.1-R8.3): every section's caption in Fork's
    /// order and, under each open one, the refs whose names hold `text` (`matching`'s rule),
    /// branches and remote-tracking refs grouped into folders by `/`, folders first at each
    /// level, and the stashes whose messages hold it. `None` when `keep_going` says to stop,
    /// asked before the first entry and every few thousand after: a pass over every ref, so
    /// a worker's.
    pub fn sidebar_rows(
        &self,
        text: &str,
        disclosure: &Disclosure,
        keep_going: impl FnMut() -> bool,
    ) -> Option<Vec<SidebarRow>> {
        let mut pass = Pass {
            keep_going,
            seen: 0,
        };
        let matched = self.matching(text, &mut pass.keep_going)?;
        let filtering = !text.is_empty();
        let mut rows = Vec::new();
        for section in SidebarSection::ALL {
            let open = filtering || disclosure.is_section_open(section);
            let caption = rows.len();
            rows.push(SidebarRow::Section { section, open });
            if !open {
                continue;
            }
            match section.kind() {
                None => {
                    for &index in &matched.stashes {
                        if !pass.tick() {
                            return None;
                        }
                        rows.push(SidebarRow::Stash { index });
                    }
                }
                Some(kind) => {
                    if section == SidebarSection::Branches
                        && matches!(self.head, HeadState::Detached(_))
                        && Folded::of(text).found_in(b"HEAD")
                    {
                        rows.push(SidebarRow::DetachedHead);
                    }
                    let listed: Vec<u32> = matched
                        .refs
                        .iter()
                        .copied()
                        .filter(|&index| {
                            self.refs
                                .get(index as usize)
                                .is_some_and(|listed| listed.kind == kind)
                        })
                        .collect();
                    let lay_out = LayOut {
                        snapshot: self,
                        disclosure,
                        filtering,
                    };
                    if section.has_folders() {
                        lay_out.level(&listed, 0, &mut rows, &mut pass)?;
                    } else {
                        let mut tags = Vec::with_capacity(listed.len());
                        for index in listed {
                            if !pass.tick() {
                                return None;
                            }
                            tags.push(index);
                        }
                        tags.sort_by(|a, b| {
                            natural_order(lay_out.short_name(*a), lay_out.short_name(*b))
                        });
                        rows.extend(
                            tags.into_iter()
                                .map(|index| SidebarRow::Ref { index, depth: 0 }),
                        );
                    }
                }
            }
            // While filtering, a section nothing matched in draws no caption.
            if filtering && rows.len() == caption + 1 {
                rows.pop();
            }
        }
        Some(rows)
    }
}

struct LayOut<'a> {
    snapshot: &'a RefsSnapshot,
    disclosure: &'a Disclosure,
    filtering: bool,
}

impl LayOut<'_> {
    /// The name of the ref at `index` past its namespace.
    fn short_name(&self, index: u32) -> &str {
        self.snapshot
            .refs
            .get(index as usize)
            .map_or("", |listed| listed.name.shorthand())
    }

    /// The `depth`th part of the name of the ref at `index`, and whether more parts follow it.
    fn part(&self, index: u32, depth: usize) -> (&str, bool) {
        let short = self.short_name(index);
        let mut parts = short.splitn(depth + 2, '/');
        let part = parts.nth(depth).unwrap_or("");
        (part, parts.next().is_some())
    }

    /// Lays out `refs` — every one of them under the same `depth` folders — folders first,
    /// each followed by what it holds when it is open, then the refs at this level.
    fn level<F: FnMut() -> bool>(
        &self,
        refs: &[u32],
        depth: usize,
        rows: &mut Vec<SidebarRow>,
        pass: &mut Pass<F>,
    ) -> Option<()> {
        let depth_row = u16::try_from(depth).unwrap_or(u16::MAX);
        let mut folders: Vec<&[u32]> = Vec::new();
        let mut leaves: Vec<u32> = Vec::new();
        let mut at = 0;
        while let Some(&index) = refs.get(at) {
            if !pass.tick() {
                return None;
            }
            let (part, deeper) = self.part(index, depth);
            if !deeper {
                leaves.push(index);
                at += 1;
                continue;
            }
            let mut end = at + 1;
            while let Some(&next) = refs.get(end) {
                let (next_part, next_deeper) = self.part(next, depth);
                if !next_deeper || next_part != part {
                    break;
                }
                if !pass.tick() {
                    return None;
                }
                end += 1;
            }
            folders.push(refs.get(at..end).unwrap_or(&[]));
            at = end;
        }
        // Folders first, then the refs, each in natural order.
        let by_part = |a: u32, b: u32| natural_order(self.part(a, depth).0, self.part(b, depth).0);
        folders.sort_by(|a, b| match (a.first(), b.first()) {
            (Some(&a), Some(&b)) => by_part(a, b),
            _ => Ordering::Equal,
        });
        leaves.sort_by(|&a, &b| by_part(a, b));
        for folder in folders {
            let Some(&first) = folder.first() else {
                continue;
            };
            let open = self.filtering
                || self
                    .disclosure
                    .is_folder_open(folder_path(self.snapshot, first, depth_row));
            rows.push(SidebarRow::Folder {
                first,
                depth: depth_row,
                open,
            });
            if open {
                self.level(folder, depth + 1, rows, pass)?;
            }
        }
        rows.extend(leaves.into_iter().map(|index| SidebarRow::Ref {
            index,
            depth: depth_row,
        }));
        Some(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::refs::{Ref, RefTarget, StashEntry};
    use crate::{Oid, RefName};

    fn oid(n: u8) -> Oid {
        Oid::from_bytes(&[n; 20]).unwrap()
    }

    fn snapshot(names: &[&str], head: HeadState, stashes: &[&str]) -> RefsSnapshot {
        let mut names: Vec<&str> = names.to_vec();
        names.sort_unstable();
        RefsSnapshot {
            refs: names
                .iter()
                .map(|name| Ref {
                    name: RefName::new(*name),
                    kind: if name.starts_with("refs/heads/") {
                        RefKind::LocalBranch
                    } else if name.starts_with("refs/remotes/") {
                        RefKind::RemoteTracking
                    } else {
                        RefKind::Tag
                    },
                    target: RefTarget::Commit(oid(1)),
                    symbolic: None,
                    upstream: None,
                })
                .collect(),
            head,
            stashes: stashes
                .iter()
                .enumerate()
                .map(|(index, message)| StashEntry {
                    index,
                    message: (*message).to_owned(),
                    commit: oid(9),
                    base: oid(1),
                })
                .collect(),
            unreadable: 0,
        }
    }

    /// Each row as text: a section by its caption, a folder by its name and whether it is
    /// open, a ref by its full name, each indented by its depth.
    fn drawn(snapshot: &RefsSnapshot, rows: &[SidebarRow]) -> Vec<String> {
        rows.iter()
            .map(|row| match *row {
                SidebarRow::Section { section, open } => format!("[{section:?}{}]", mark(open)),
                SidebarRow::Folder { first, depth, open } => format!(
                    "{}{}/{}",
                    "  ".repeat(usize::from(depth)),
                    folder_name(snapshot, first, depth),
                    mark(open)
                ),
                SidebarRow::Ref { index, depth } => format!(
                    "{}{}",
                    "  ".repeat(usize::from(depth)),
                    snapshot.refs[index as usize].name.as_str()
                ),
                SidebarRow::DetachedHead => "HEAD".to_owned(),
                SidebarRow::Stash { index } => {
                    format!("stash {}", snapshot.stashes[index as usize].message)
                }
            })
            .collect()
    }

    fn mark(open: bool) -> &'static str {
        if open { " open" } else { "" }
    }

    const NAMES: &[&str] = &[
        "refs/heads/main",
        "refs/heads/feature/login",
        "refs/heads/feature/ui/menu",
        "refs/heads/feature-x",
        "refs/heads/zeta",
        "refs/heads/a/b",
        "refs/remotes/origin/main",
        "refs/remotes/origin/feature/login",
        "refs/remotes/upstream/main",
        "refs/tags/v1.0",
        "refs/tags/release/2.0",
    ];

    /// R8.1, R8.2: the sections in Fork's order, every one drawn; branches and remotes in
    /// folders split at `/`, folders first at each level then the refs, each in name order; a
    /// closed folder hides what it holds; tags never foldered; every stash listed. Caught
    /// by: sections out of order or one missing, a folder's refs drawn under a closed folder,
    /// folders after the refs of their level, tags grouped, or a folder split in two.
    #[test]
    fn sections_come_in_forks_order_and_branches_and_remotes_fold_at_slashes() {
        let snapshot = snapshot(
            NAMES,
            HeadState::Branch(RefName::new("refs/heads/main")),
            &["On main: wip", "On main: wip"],
        );
        let mut disclosure = Disclosure::default();
        disclosure.toggle_folder("refs/heads/feature");
        disclosure.toggle_folder("refs/remotes/origin");
        let rows = snapshot.sidebar_rows("", &disclosure, || true).unwrap();
        assert_eq!(
            drawn(&snapshot, &rows),
            [
                "[Branches open]",
                "a/",
                "feature/ open",
                "  ui/",
                "  refs/heads/feature/login",
                "refs/heads/feature-x",
                "refs/heads/main",
                "refs/heads/zeta",
                "[Remotes open]",
                "origin/ open",
                "  feature/",
                "  refs/remotes/origin/main",
                "upstream/",
                "[Tags open]",
                "refs/tags/release/2.0",
                "refs/tags/v1.0",
                "[Stashes open]",
                "stash On main: wip",
                "stash On main: wip",
            ]
        );
    }

    /// R8.3 and the user's decision (2026-10-07): the filter keeps the refs whose name holds
    /// the text and the stashes whose message does, and draws every section and folder open —
    /// a closed one too — so a match is never hidden; a section nothing matched in draws no
    /// caption. Caught by: matches left inside a closed folder or section, or a caption drawn
    /// over nothing.
    #[test]
    fn a_filter_keeps_what_matches_with_every_section_and_folder_open() {
        let mut names = NAMES.to_vec();
        names.push("refs/tags/login-v1");
        let snapshot = snapshot(
            &names,
            HeadState::Branch(RefName::new("refs/heads/main")),
            &["On main: login wip"],
        );
        let mut disclosure = Disclosure::default();
        disclosure.toggle_section(SidebarSection::Tags);
        let rows = snapshot
            .sidebar_rows("LOGIN", &disclosure, || true)
            .unwrap();
        assert_eq!(
            drawn(&snapshot, &rows),
            [
                "[Branches open]",
                "feature/ open",
                "  refs/heads/feature/login",
                "[Remotes open]",
                "origin/ open",
                "  feature/ open",
                "    refs/remotes/origin/feature/login",
                "[Tags open]",
                "refs/tags/login-v1",
                "[Stashes open]",
                "stash On main: login wip",
            ]
        );
        let rows = snapshot.sidebar_rows("v1.0", &disclosure, || true).unwrap();
        assert_eq!(
            drawn(&snapshot, &rows),
            ["[Tags open]", "refs/tags/v1.0"],
            "a section nothing matched in drew its caption"
        );
        // With no text, the closed section is closed again.
        let rows = snapshot.sidebar_rows("", &disclosure, || true).unwrap();
        assert!(drawn(&snapshot, &rows).contains(&"[Tags]".to_owned()));
    }

    /// The user's decision (2026-10-07): Fork's natural order, folders first — at each level
    /// folders, then refs, each with case ignored and numbers read as numbers; tags the same.
    /// Caught by: bytewise order (`Gamma` before `alpha`, `b10` before `b2`), or folders mixed
    /// in among the refs.
    #[test]
    fn each_level_is_in_natural_order_folders_first() {
        let snapshot = snapshot(
            &[
                "refs/heads/b10",
                "refs/heads/b2",
                "refs/heads/Gamma",
                "refs/heads/alpha",
                "refs/heads/Beta",
                "refs/heads/Zone/x",
                "refs/heads/area/y",
                "refs/tags/v1.10",
                "refs/tags/V1.2",
                "refs/tags/v1.9",
            ],
            HeadState::Branch(RefName::new("refs/heads/alpha")),
            &[],
        );
        let rows = snapshot
            .sidebar_rows("", &Disclosure::default(), || true)
            .unwrap();
        assert_eq!(
            drawn(&snapshot, &rows),
            [
                "[Branches open]",
                "area/",
                "Zone/",
                "refs/heads/alpha",
                "refs/heads/b2",
                "refs/heads/b10",
                "refs/heads/Beta",
                "refs/heads/Gamma",
                "[Remotes open]",
                "[Tags open]",
                "refs/tags/V1.2",
                "refs/tags/v1.9",
                "refs/tags/v1.10",
                "[Stashes open]",
            ]
        );
    }

    /// The order itself: case ignored, a digit run read as its number, and a total order for
    /// names alike but for case or leading zeros. Caught by: a bytewise comparison, digits
    /// compared as text, or two different names called equal.
    #[test]
    fn natural_order_reads_numbers_as_numbers_and_ignores_case() {
        use std::cmp::Ordering::{Greater, Less};
        assert_eq!(natural_order("b2", "b10"), Less);
        assert_eq!(natural_order("b10", "b9"), Greater);
        assert_eq!(natural_order("alpha", "Beta"), Less);
        assert_eq!(natural_order("Gamma", "beta"), Greater);
        assert_eq!(natural_order("release-2", "release-10"), Less);
        assert_eq!(natural_order("v1.2.3", "v1.10.0"), Less);
        assert_eq!(natural_order("a", "a1"), Less);
        assert_eq!(natural_order("Main", "main"), "Main".cmp("main"));
        assert_eq!(natural_order("a01", "a1"), "a01".cmp("a1"));
    }

    /// Fork's detached `HEAD`: the first row under Branches, kept by a filter that `HEAD`
    /// holds and no other. Caught by: no row for a detached `HEAD`, one for a branch's, or one
    /// that ignores the filter.
    #[test]
    fn a_detached_head_is_the_first_branch_row() {
        let detached = snapshot(NAMES, HeadState::Detached(oid(1)), &[]);
        let rows = detached
            .sidebar_rows("", &Disclosure::default(), || true)
            .unwrap();
        assert_eq!(rows.get(1), Some(&SidebarRow::DetachedHead));
        let rows = detached
            .sidebar_rows("hea", &Disclosure::default(), || true)
            .unwrap();
        assert_eq!(rows.get(1), Some(&SidebarRow::DetachedHead));
        let rows = detached
            .sidebar_rows("main", &Disclosure::default(), || true)
            .unwrap();
        assert!(!rows.contains(&SidebarRow::DetachedHead));
        let on_main = snapshot(
            NAMES,
            HeadState::Branch(RefName::new("refs/heads/main")),
            &[],
        );
        let rows = on_main
            .sidebar_rows("", &Disclosure::default(), || true)
            .unwrap();
        assert!(!rows.contains(&SidebarRow::DetachedHead));
    }

    /// A folder is opened and closed by its full path, and the current branch's folders are
    /// revealed — every one it sits in, and nothing that is not a folder. Caught by: a path
    /// cut at the wrong `/`, the namespace taken for a folder, or a reveal that opens the
    /// branch itself.
    #[test]
    fn a_folder_is_named_by_its_path_and_a_branch_reveals_its_folders() {
        let snapshot = snapshot(
            NAMES,
            HeadState::Branch(RefName::new("refs/heads/main")),
            &[],
        );
        let menu = snapshot
            .refs
            .iter()
            .position(|listed| listed.name.as_str() == "refs/heads/feature/ui/menu")
            .unwrap() as u32;
        assert_eq!(folder_name(&snapshot, menu, 0), "feature");
        assert_eq!(folder_name(&snapshot, menu, 1), "ui");
        assert_eq!(folder_path(&snapshot, menu, 0), "refs/heads/feature");
        assert_eq!(folder_path(&snapshot, menu, 1), "refs/heads/feature/ui");
        let origin = snapshot
            .refs
            .iter()
            .position(|listed| listed.name.as_str() == "refs/remotes/origin/main")
            .unwrap() as u32;
        assert_eq!(folder_path(&snapshot, origin, 0), "refs/remotes/origin");

        let mut disclosure = Disclosure::default();
        assert!(disclosure.reveal("refs/heads/feature/ui/menu"));
        assert!(disclosure.is_folder_open("refs/heads/feature"));
        assert!(disclosure.is_folder_open("refs/heads/feature/ui"));
        assert!(!disclosure.is_folder_open("refs/heads/feature/ui/menu"));
        assert!(!disclosure.is_folder_open("refs/heads"));
        assert!(
            !disclosure.reveal("refs/heads/feature/ui/menu"),
            "nothing new"
        );
        assert!(!disclosure.reveal("refs/heads/main"));
        disclosure.toggle_folder("refs/heads/feature");
        assert!(!disclosure.is_folder_open("refs/heads/feature"));
    }

    /// A stop asked for is honoured mid-pass, however the refs are laid out. Caught by: a
    /// layout that never asks, or runs on after it was told to stop.
    #[test]
    fn a_stop_is_honoured_mid_pass() {
        let names: Vec<String> = (0..10_000)
            .map(|n| format!("refs/heads/team/branch-{n:05}"))
            .collect();
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        let snapshot = snapshot(
            &names,
            HeadState::Branch(RefName::new("refs/heads/team/branch-00000")),
            &[],
        );
        let mut disclosure = Disclosure::default();
        disclosure.toggle_folder("refs/heads/team");
        let full = snapshot.sidebar_rows("", &disclosure, || true).unwrap();
        assert_eq!(full.len(), 10_000 + 1 + 4);
        let mut asked = 0;
        assert_eq!(
            snapshot.sidebar_rows("", &disclosure, || {
                asked += 1;
                asked < 2
            }),
            None
        );
        assert_eq!(snapshot.sidebar_rows("", &disclosure, || false), None);
    }
}
