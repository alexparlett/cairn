//! The chrome's icons: Lucide's, compiled in by Freya's `icons-lucide` feature (ISC; the
//! feature and the licence recorded in `deny.toml`). This is the one module that names the
//! icon set (`every_icon_is_named_through_the_icon_module`): a view asks for an icon by what it
//! means, at a size and in a tone, and never for a file of the set.

use freya::icons::lucide;
use freya::prelude::*;

/// The branch glyph: a branch's row in the sidebar, the status box's branch.
pub fn branch(size: f32, colour: Color) -> Element {
    drawn(("git-branch", lucide::git_branch()), size, colour)
}

/// A remote's cloud.
pub fn cloud(size: f32, colour: Color) -> Element {
    drawn(("cloud", lucide::cloud()), size, colour)
}

/// A stash's archive box.
pub fn archive(size: f32, colour: Color) -> Element {
    drawn(("archive", lucide::archive()), size, colour)
}

/// An open folder: a folder of branches or remotes.
pub fn folder(size: f32, colour: Color) -> Element {
    drawn(("folder-open", lucide::folder_open()), size, colour)
}

/// The gear whose menu holds a repository's settings and paths.
pub fn gear(size: f32, colour: Color) -> Element {
    drawn(("settings", lucide::settings()), size, colour)
}

/// The magnifier in a filter field.
pub fn magnifier(size: f32, colour: Color) -> Element {
    drawn(("search", lucide::search()), size, colour)
}

/// The sidebar toggle.
pub fn sidebar(size: f32, colour: Color) -> Element {
    drawn(("panel-left", lucide::panel_left()), size, colour)
}

/// Quick Launch's folder.
pub fn quick_launch(size: f32, colour: Color) -> Element {
    drawn(("folder", lucide::folder()), size, colour)
}

/// Fetch: an arrow down to a remote's point.
pub fn fetch(size: f32, colour: Color) -> Element {
    drawn(
        ("arrow-down-to-dot", lucide::arrow_down_to_dot()),
        size,
        colour,
    )
}

/// Pull: an arrow down to a line.
pub fn pull(size: f32, colour: Color) -> Element {
    drawn(
        ("arrow-down-to-line", lucide::arrow_down_to_line()),
        size,
        colour,
    )
}

/// Push: an arrow up from a line.
pub fn push(size: f32, colour: Color) -> Element {
    drawn(
        ("arrow-up-from-line", lucide::arrow_up_from_line()),
        size,
        colour,
    )
}

/// A new branch: the branch glyph with a plus, Fork's Branch button.
pub fn new_branch(size: f32, colour: Color) -> Element {
    drawn(("git-branch-plus", lucide::git_branch_plus()), size, colour)
}

/// Appearance: a window split in two panes.
pub fn appearance(size: f32, colour: Color) -> Element {
    drawn(("columns-2", lucide::columns_2()), size, colour)
}

/// The ⌄ beside a button whose menu opens on its own press.
pub fn chevron_down(size: f32, colour: Color) -> Element {
    drawn(("chevron-down", lucide::chevron_down()), size, colour)
}

/// The status box's list glyph, which opens the activity popover.
pub fn activity(size: f32, colour: Color) -> Element {
    drawn(
        ("text-align-start", lucide::text_align_start()),
        size,
        colour,
    )
}

/// The detail pane's Hide commit details: a window with its bottom pane.
pub fn bottom_pane(size: f32, colour: Color) -> Element {
    drawn(("panel-bottom", lucide::panel_bottom()), size, colour)
}

/// The ⌄'s sideways twin: a closed section or folder of the sidebar.
pub fn chevron_right(size: f32, colour: Color) -> Element {
    drawn(("chevron-right", lucide::chevron_right()), size, colour)
}

/// A tag's outline.
pub fn tag(size: f32, colour: Color) -> Element {
    drawn(("tag", lucide::tag()), size, colour)
}

/// The check mark the current branch draws in place of its glyph.
pub fn check(size: f32, colour: Color) -> Element {
    drawn(("check", lucide::check()), size, colour)
}

/// A branch with no upstream: a line of commits that does not fork.
pub fn local_branch(size: f32, colour: Color) -> Element {
    drawn(
        ("git-commit-vertical", lucide::git_commit_vertical()),
        size,
        colour,
    )
}

/// A warning: a branch whose upstream is gone.
pub fn warning(size: f32, colour: Color) -> Element {
    drawn(("triangle-alert", lucide::triangle_alert()), size, colour)
}

/// Local Changes: a document with a plus over a minus.
pub fn local_changes(size: f32, colour: Color) -> Element {
    drawn(("file-diff", lucide::file_diff()), size, colour)
}

/// All Commits: a stack of pages.
pub fn all_commits(size: f32, colour: Color) -> Element {
    drawn(("file-stack", lucide::file_stack()), size, colour)
}

/// One icon, square, its strokes in `colour`: drawn at once, without a loader's flash, since
/// the set is compiled in.
fn drawn(source: impl Into<ImageSource>, size: f32, colour: Color) -> Element {
    SvgViewer::new(source)
        .show_loader(false)
        .color(colour)
        .width(Size::px(size))
        .height(Size::px(size))
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An icon's name in the set and its SVG's bytes.
    type Source = (&'static str, fn() -> Vec<u8>);

    /// Every icon named here is one of the set's SVGs, compiled in. Caught by: the feature
    /// turned off, or a name that resolves to something other than an SVG.
    #[test]
    fn every_icon_is_an_svg_of_the_set() {
        let sources: [Source; 23] = [
            ("git-branch", || lucide::git_branch().to_vec()),
            ("cloud", || lucide::cloud().to_vec()),
            ("archive", || lucide::archive().to_vec()),
            ("folder-open", || lucide::folder_open().to_vec()),
            ("settings", || lucide::settings().to_vec()),
            ("search", || lucide::search().to_vec()),
            ("panel-left", || lucide::panel_left().to_vec()),
            ("folder", || lucide::folder().to_vec()),
            ("arrow-down-to-dot", || lucide::arrow_down_to_dot().to_vec()),
            ("arrow-down-to-line", || {
                lucide::arrow_down_to_line().to_vec()
            }),
            ("arrow-up-from-line", || {
                lucide::arrow_up_from_line().to_vec()
            }),
            ("git-branch-plus", || lucide::git_branch_plus().to_vec()),
            ("columns-2", || lucide::columns_2().to_vec()),
            ("chevron-down", || lucide::chevron_down().to_vec()),
            ("text-align-start", || lucide::text_align_start().to_vec()),
            ("panel-bottom", || lucide::panel_bottom().to_vec()),
            ("chevron-right", || lucide::chevron_right().to_vec()),
            ("tag", || lucide::tag().to_vec()),
            ("check", || lucide::check().to_vec()),
            ("git-commit-vertical", || {
                lucide::git_commit_vertical().to_vec()
            }),
            ("triangle-alert", || lucide::triangle_alert().to_vec()),
            ("file-diff", || lucide::file_diff().to_vec()),
            ("file-stack", || lucide::file_stack().to_vec()),
        ];
        for (name, source) in sources {
            let svg = String::from_utf8(source()).unwrap_or_default();
            assert!(svg.contains("<svg"), "{name} is not an SVG");
            assert!(
                svg.contains("currentColor"),
                "{name} does not take its colour"
            );
        }
    }
}
