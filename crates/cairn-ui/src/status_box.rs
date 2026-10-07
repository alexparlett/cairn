//! The title bar's status box, Fork's (refs-and-status R7.1; `fork-refs-and-status-ui.md`,
//! section 9): the repository's name — its folder's, as the worker that opened it names it — marked `*` while status reports a change; a branch
//! glyph and the current branch; and how far it is from its upstream — behind, then ahead,
//! Fork's `18↓ 1↑`, a count of nothing left out as Fork leaves it — or that its upstream is
//! gone. A detached `HEAD` names its short id, an unborn branch its name and that it has no
//! commit yet, each in git's own words. The arrows are drawn in IBM Plex Mono, the typeface
//! the application embeds, which carries them.

use cairn_model::{AheadBehind, HeadState, RefName};
use freya::prelude::*;

use crate::diff_palette::DIFF_FONT_FAMILY;
use crate::ref_glyphs::RefGlyph;

/// Said after an unborn branch's name: `git status`'s "No commits yet".
pub const NO_COMMITS_YET: &str = "no commits yet";
/// Said for a branch whose upstream is configured but gone: git's `[gone]`.
pub const UPSTREAM_GONE: &str = "upstream gone";

const FONT_SIZE: f32 = 13.0;

/// Where the current branch stands against its upstream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tracking {
    /// No upstream, `HEAD` on no branch, or the counts not read yet: nothing is said.
    Untold,
    /// The counts of the branch against its upstream.
    Counts(AheadBehind),
    /// The upstream is configured but no such ref exists.
    Gone,
}

/// What the title bar says of `HEAD`: the branch's name, `HEAD detached at <short id>`, or
/// an unborn branch's name and that it has no commit.
pub fn head_text(head: &HeadState) -> String {
    match head {
        HeadState::Branch(name) => name.shorthand().to_owned(),
        HeadState::Detached(id) => format!("HEAD detached at {}", id.short().as_str()),
        HeadState::Unborn(name) => format!("{} ({NO_COMMITS_YET})", name.shorthand()),
    }
}

/// `n↓ m↑` as Fork prints it, behind first, a zero count left out; empty when the branch is
/// level with its upstream.
pub fn counts_text(counts: AheadBehind) -> String {
    let behind = (counts.behind > 0).then(|| format!("{}↓", counts.behind));
    let ahead = (counts.ahead > 0).then(|| format!("{}↑", counts.ahead));
    behind
        .into_iter()
        .chain(ahead)
        .collect::<Vec<_>>()
        .join(" ")
}

/// The repository's name, `*` after it while the working tree has changes.
pub fn name_text(name: &str, dirty: bool) -> String {
    if dirty {
        format!("{name}*")
    } else {
        name.to_owned()
    }
}

/// The status box: no handler, so equal content compares equal and is not redrawn.
#[derive(Debug, Clone, PartialEq)]
pub struct StatusBox {
    name: Option<String>,
    dirty: bool,
    head: Option<HeadState>,
    tracking: Tracking,
    key: DiffKey,
}

impl StatusBox {
    /// The box for the repository called `name` — `None` until it is open — and `head` —
    /// `None` until the refs are read.
    pub fn new(name: Option<String>, head: Option<HeadState>) -> Self {
        Self {
            name,
            dirty: false,
            head,
            tracking: Tracking::Untold,
            key: DiffKey::None,
        }
    }

    /// Whether the last status read listed any change.
    pub fn dirty(mut self, dirty: bool) -> Self {
        self.dirty = dirty;
        self
    }

    pub fn tracking(mut self, tracking: Tracking) -> Self {
        self.tracking = tracking;
        self
    }
}

impl KeyExt for StatusBox {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl ComponentOwned for StatusBox {
    fn render(self) -> impl IntoElement {
        let colours = get_theme_or_default();
        let primary = colours.read().colors().text_primary;
        let secondary = colours.read().colors().text_secondary;
        let warning = colours.read().colors().warning;
        let on_branch = match &self.head {
            Some(HeadState::Branch(_) | HeadState::Unborn(_)) => true,
            Some(HeadState::Detached(_)) | None => false,
        };
        let tracked = match self.tracking {
            Tracking::Untold => None,
            Tracking::Counts(counts) => Some(counts_text(counts))
                .filter(|text| !text.is_empty())
                .map(|text| {
                    label()
                        .text(text)
                        .max_lines(1)
                        .font_family(DIFF_FONT_FAMILY)
                        .font_size(FONT_SIZE)
                        .color(secondary)
                }),
            Tracking::Gone => Some(
                label()
                    .text(UPSTREAM_GONE)
                    .max_lines(1)
                    .font_size(FONT_SIZE)
                    .color(warning),
            ),
        };
        rect()
            .horizontal()
            .cross_align(Alignment::center())
            .spacing(8.)
            .maybe_child(self.name.as_deref().map(|name| {
                label()
                    .text(name_text(name, self.dirty))
                    .max_lines(1)
                    .font_size(FONT_SIZE)
                    .font_weight(FontWeight::BOLD)
                    .color(primary)
            }))
            .maybe_child(on_branch.then(|| RefGlyph::Branch.draw(secondary)))
            .maybe_child(self.head.as_ref().map(|head| {
                label()
                    .text(head_text(head))
                    .max_lines(1)
                    .font_size(FONT_SIZE)
                    .color(primary)
            }))
            .maybe_child(tracked)
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}

/// The branch `HEAD` is on, when it has a commit: the one whose upstream the box measures.
pub fn current_branch(head: &HeadState) -> Option<&RefName> {
    match head {
        HeadState::Branch(name) => Some(name),
        HeadState::Detached(_) | HeadState::Unborn(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cairn_model::Oid;

    /// R7.1 in git's words: a branch by its name, a detached `HEAD` by its short id, an unborn
    /// branch by its name and that it has no commit. Caught by: a full ref name, the long id,
    /// or an unborn branch drawn as an ordinary one.
    #[test]
    fn head_is_named_as_git_names_it() {
        let id = Oid::from_bytes(&[0xab; 20]).unwrap();
        assert_eq!(
            head_text(&HeadState::Branch(RefName::new("refs/heads/feature/x"))),
            "feature/x"
        );
        assert_eq!(
            head_text(&HeadState::Detached(id)),
            "HEAD detached at abababa"
        );
        assert_eq!(
            head_text(&HeadState::Unborn(RefName::new("refs/heads/main"))),
            "main (no commits yet)"
        );
    }

    /// Fork's counts: behind then ahead, each with its arrow after it, a zero left out, and
    /// nothing at all for a branch level with its upstream. Caught by: the two swapped, a
    /// zero drawn, or an arrow on the wrong count.
    #[test]
    fn counts_are_behind_then_ahead_and_a_zero_is_left_out() {
        let counted = |ahead, behind| AheadBehind { ahead, behind };
        assert_eq!(counts_text(counted(1, 18)), "18↓ 1↑");
        assert_eq!(counts_text(counted(0, 853)), "853↓");
        assert_eq!(counts_text(counted(2, 0)), "2↑");
        assert_eq!(counts_text(counted(0, 0)), "");
    }

    #[test]
    fn a_dirty_repository_is_starred() {
        assert_eq!(name_text("swift", true), "swift*");
        assert_eq!(name_text("swift", false), "swift");
    }
}
