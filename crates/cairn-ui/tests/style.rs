//! The style module holds Fork's sizes and Fork Dark's tones, and the theme the window
//! launches with carries them into Freya's own components.

use cairn_ui::style::{metrics, tones};
use freya::prelude::{
    ButtonColorsThemePreference, Color, InputColorsThemePreference, MenuContainerThemePreference,
    MenuItemThemePreference, Preference, TooltipThemePreference, rect,
};
use freya_testing::launch_test;

fn colour(hex: &str) -> Color {
    Color::from_hex(hex).unwrap_or_else(|| panic!("{hex} is not a colour"))
}

/// Every row of the PRD's metric and tone tables, spelled out here as the tables write them, is
/// the module's value. Caught by: a size or a tone typed in wrong, or one changed without the
/// table it comes from.
#[test]
fn the_style_module_holds_forks_metrics_and_fork_darks_tones() {
    for (row, value, expected) in [
        ("Body text", metrics::BODY, 12.),
        ("Toolbar labels", metrics::TOOLBAR_LABEL, 11.),
        ("Caption row", metrics::CAPTION_ROW, 26.),
        ("Toolbar", metrics::TOOLBAR, 41.),
        ("History row", metrics::HISTORY_ROW, 23.),
        ("Sidebar row", metrics::SIDEBAR_ROW, 22.),
        ("Local Changes row", metrics::LOCAL_CHANGES_ROW, 20.),
        ("Menu item", metrics::MENU_ITEM, 23.),
        ("Sidebar default width", metrics::SIDEBAR_WIDTH, 300.),
        ("Indent per sidebar level", metrics::SIDEBAR_INDENT, 12.),
        ("Diff row", metrics::DIFF_ROW, 17.),
    ] {
        assert_eq!(value, expected, "{row}");
    }

    for (row, value, expected) in [
        ("Caption row, menu bar, toolbar", tones::CHROME, "#1A1A1A"),
        ("Sidebar", tones::SIDEBAR, "#2C2C2D"),
        ("History", tones::HISTORY, "#212121"),
        ("Detail pane", tones::DETAIL, "#333333"),
        (
            "Detail tab strip, list headings",
            tones::HEADING_BAR,
            "#2B2B2B",
        ),
        ("Diff body", tones::DIFF_BODY, "#282828"),
        ("Rules and splitters", tones::RULE, "#525252"),
        ("Body text", tones::TEXT, "#EAE8E8"),
        ("Toolbar labels", tones::TOOLBAR_TEXT, "#B2B2B2"),
        ("Detail labels", tones::DETAIL_LABEL, "#C4C0C0"),
        ("Rows HEAD cannot reach", tones::UNREACHABLE_TEXT, "#A9A9A9"),
        ("Disabled", tones::DISABLED_TEXT, "#707070"),
        ("Disabled, on", tones::DISABLED_FILL, "#343434"),
        (
            "Selection, its list focused",
            tones::SELECTION_FOCUSED,
            "#236BD2",
        ),
        (
            "Selection, its list focused: white text",
            tones::SELECTED_TEXT,
            "#FFFFFF",
        ),
        (
            "Selection, its list unfocused",
            tones::SELECTION_UNFOCUSED,
            "#525252",
        ),
        ("Menu", tones::MENU, "#2C2C2C"),
        ("Menu, border", tones::MENU_BORDER, "#525252"),
        ("Menu item, hovered", tones::MENU_HOVER, "#424242"),
        ("Menu separator", tones::MENU_SEPARATOR, "#525252"),
        ("Menu chord text", tones::MENU_CHORD, "#9A9797"),
        ("Dialog", tones::DIALOG, "#2B2B2B"),
        ("Dialog, border", tones::DIALOG_BORDER, "#3BACED"),
        ("Text field, focused", tones::ACCENT, "#0078D7"),
        ("Button", tones::BUTTON, "#373737"),
        ("Button, border", tones::BUTTON_BORDER, "#525252"),
        ("Added line", tones::ADDED_LINE, "#3A5C3F"),
        ("Removed line", tones::REMOVED_LINE, "#633F3E"),
        ("Changed word", tones::CHANGED_WORD, "#9F4247"),
        ("Hunk header text", tones::HUNK_HEADER_TEXT, "#959595"),
        ("Chip: tag", tones::TAG_CHIP, "#3E3F4E"),
        ("Chip: tag, border", tones::TAG_CHIP_BORDER, "#636281"),
        ("Chip: stash", tones::STASH_CHIP, "#545454"),
        ("Chip: stash, border", tones::STASH_CHIP_BORDER, "#929292"),
        ("Chip text", tones::CHIP_TEXT, "#F0F0F0"),
    ] {
        assert_eq!(value, colour(expected), "{row}");
    }

    // A selected row: blue with white text while its list has focus, grey with the body's
    // text when not.
    let focused = tones::selected(true);
    assert_eq!(
        (focused.band, focused.text),
        (colour("#236BD2"), colour("#FFFFFF"))
    );
    let unfocused = tones::selected(false);
    assert_eq!(
        (unfocused.band, unfocused.text),
        (colour("#525252"), colour("#EAE8E8"))
    );
}

/// The theme the window launches with puts Fork Dark's tones where Freya's components read
/// theirs — the sheet's slots, the buttons, the menus, the fields — and gives a tooltip the
/// body's size rather than Freya's 14 px. Caught by: the launch theme left at Freya's stock
/// tones, or a component that falls back to them.
#[test]
fn the_launch_theme_carries_fork_darks_tones_into_freyas_components() {
    // Freya builds its dark theme inside a running window, which reads the desktop's accent.
    let theme = launch_test(rect).run_in(tones::theme);
    let sheet = theme.colors();
    assert_eq!(sheet.text_primary, tones::TEXT);
    assert_eq!(sheet.background, tones::HISTORY);
    assert_eq!(sheet.border, tones::RULE);
    assert_eq!(sheet.border_focus, tones::ACCENT);
    assert_eq!(sheet.surface_tertiary, tones::BUTTON);
    assert_eq!(sheet.surface_secondary, tones::MENU_HOVER);
    assert_eq!(sheet.primary, tones::SELECTION_FOCUSED);
    assert_eq!(sheet.disabled, tones::DISABLED_TEXT);

    let specific = |colour: Color| Preference::Specific(colour);
    let button = theme
        .get::<ButtonColorsThemePreference>("button")
        .unwrap_or_else(|| panic!("no button theme"));
    assert_eq!(button.background, specific(tones::BUTTON));
    assert_eq!(button.border_fill, specific(tones::BUTTON_BORDER));
    assert_eq!(button.disabled_background, specific(tones::DISABLED_FILL));
    assert_eq!(button.disabled_color, specific(tones::DISABLED_TEXT));

    let menu = theme
        .get::<MenuContainerThemePreference>("menu_container")
        .unwrap_or_else(|| panic!("no menu theme"));
    assert_eq!(menu.background, specific(tones::MENU));
    assert_eq!(menu.border_fill, specific(tones::MENU_BORDER));
    let item = theme
        .get::<MenuItemThemePreference>("menu_item")
        .unwrap_or_else(|| panic!("no menu item theme"));
    assert_eq!(item.hover_background, specific(tones::MENU_HOVER));
    assert_eq!(item.color, specific(tones::TEXT));

    let field = theme
        .get::<InputColorsThemePreference>("input")
        .unwrap_or_else(|| panic!("no field theme"));
    assert_eq!(field.focus_border_fill, specific(tones::ACCENT));

    let tooltip = theme
        .get::<TooltipThemePreference>("tooltip")
        .unwrap_or_else(|| panic!("no tooltip theme"));
    assert_eq!(tooltip.font_size, Preference::Specific(metrics::BODY));
}
