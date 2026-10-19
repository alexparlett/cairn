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
        let sources: [Source; 7] = [
            ("git-branch", || lucide::git_branch().to_vec()),
            ("cloud", || lucide::cloud().to_vec()),
            ("archive", || lucide::archive().to_vec()),
            ("folder-open", || lucide::folder_open().to_vec()),
            ("settings", || lucide::settings().to_vec()),
            ("search", || lucide::search().to_vec()),
            ("panel-left", || lucide::panel_left().to_vec()),
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
