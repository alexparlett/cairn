//! Fork Dark's tones, sampled from Fork for Windows 2.23.2's dark theme at 1024×768
//! (`docs/research/ui-review/fork-window-survey-2-2026-10-16.md`, section 1), and the theme
//! the window launches with, built from Freya's `dark_theme` with them ([`theme`]).
//!
//! A component reads the tone it means by name here; the theme carries the same tones into
//! Freya's own components — buttons, menus, fields, tooltips — so neither draws a stock tone
//! for anything Fork Dark gives. The lanes keep Cairn's eight (`history::lane_palette`).

use freya::prelude::{
    ButtonColorsThemePreference, Color, ColorsSheet, CornerRadius, DARK_COLORS, Gaps,
    InputColorsThemePreference, MenuContainerThemePreference, MenuItemThemePreference,
    PopupThemePreference, Preference, ResizableHandleThemePreference, Theme,
    TooltipThemePreference, dark_theme,
};

/// `0xRRGGBB` as a colour.
const fn hex(rgb: u32) -> Color {
    Color::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

// Surfaces.

/// The caption row, the menu bar and the toolbar: one band.
pub const CHROME: Color = hex(0x1A1A1A);
/// The sidebar.
pub const SIDEBAR: Color = hex(0x2C2C2D);
/// The history, and every list body Fork draws on the same ground: the Changes tab's files
/// and Local Changes' lists.
pub const HISTORY: Color = hex(0x212121);
/// The detail pane's body, its summary bar and the diff's bar.
pub const DETAIL: Color = hex(0x333333);
/// The detail pane's tab strip, a list's heading row and the commit box's area.
pub const HEADING_BAR: Color = hex(0x2B2B2B);
/// The diff's body.
pub const DIFF_BODY: Color = hex(0x282828);
/// Rules and splitters, 1 px.
pub const RULE: Color = hex(0x525252);

// Text.

/// Body text.
pub const TEXT: Color = hex(0xEAE8E8);
/// The toolbar's labels: dimmer than the menus.
pub const TOOLBAR_TEXT: Color = hex(0xB2B2B2);
/// The detail pane's labels (AUTHOR, the date, the email), and text set back from the body.
pub const DETAIL_LABEL: Color = hex(0xC4C0C0);
/// The rows `HEAD` cannot reach.
pub const UNREACHABLE_TEXT: Color = hex(0xA9A9A9);
/// Disabled text.
pub const DISABLED_TEXT: Color = hex(0x707070);
/// What disabled text stands on: a disabled button's fill.
pub const DISABLED_FILL: Color = hex(0x343434);

// Selection, menus and dialogs.

/// A selected row's band while its list has focus.
pub const SELECTION_FOCUSED: Color = hex(0x236BD2);
/// A selected row's text while its list has focus: white, and bold.
pub const SELECTED_TEXT: Color = hex(0xFFFFFF);
/// A selected row's band while its list does not have focus; its text keeps the body's tone,
/// still bold.
pub const SELECTION_UNFOCUSED: Color = hex(0x525252);
/// A menu's ground.
pub const MENU: Color = hex(0x2C2C2C);
/// A menu's 1 px border.
pub const MENU_BORDER: Color = hex(0x525252);
/// A menu item under the pointer.
pub const MENU_HOVER: Color = hex(0x424242);
/// The rule between a menu's groups.
pub const MENU_SEPARATOR: Color = hex(0x525252);
/// A menu item's chord, right-aligned.
pub const MENU_CHORD: Color = hex(0x9A9797);
/// A dialog's ground.
pub const DIALOG: Color = hex(0x2B2B2B);
/// A dialog's 1 px border.
pub const DIALOG_BORDER: Color = hex(0x3BACED);
/// Fork's blue: a focused text field's border, and the chosen tab's caption in the detail
/// pane's strip.
pub const ACCENT: Color = hex(0x0078D7);
/// A button's fill.
pub const BUTTON: Color = hex(0x373737);
/// A button's 1 px border.
pub const BUTTON_BORDER: Color = hex(0x525252);

// The diff's bands and hunk header, read by `diff::palette`.

/// An added line's band.
pub const ADDED_LINE: Color = hex(0x3A5C3F);
/// A removed line's band.
pub const REMOVED_LINE: Color = hex(0x633F3E);
/// A changed word on a removed line.
pub const CHANGED_WORD: Color = hex(0x9F4247);
/// A hunk header's text.
pub const HUNK_HEADER_TEXT: Color = hex(0x959595);

// The chips Fork tones by kind. A branch's and a remote's take their row's lane instead.

/// A tag's chip.
pub const TAG_CHIP: Color = hex(0x3E3F4E);
/// A tag's chip's border.
pub const TAG_CHIP_BORDER: Color = hex(0x636281);
/// A stash's chip.
pub const STASH_CHIP: Color = hex(0x545454);
/// A stash's chip's border.
pub const STASH_CHIP_BORDER: Color = hex(0x929292);
/// A chip's text.
pub const CHIP_TEXT: Color = hex(0xF0F0F0);
/// The cloud in a remote's chip's icon segment, and in the cloud-only chip of a branch level
/// with its upstream: grey (`~#777676`).
pub const REMOTE_GLYPH: Color = hex(0x777676);

/// An author's avatar: its initials, white, on one of [`AVATARS`]. Fork's one sampled avatar is
/// lavender (`#D376E9`..`#DE9FF0`, the first); the rest are Cairn's, each dark enough under
/// white initials, chosen from the author's name.
pub const AVATAR_TEXT: Color = hex(0xFFFFFF);
/// The fixed set an avatar's colour is chosen from, by the author's name.
pub const AVATARS: [Color; 8] = [
    hex(0xD88AEC),
    hex(0x5B8FD6),
    hex(0x3E9E86),
    hex(0xD9824B),
    hex(0xC85A72),
    hex(0x7F9147),
    hex(0x7A6FD8),
    hex(0xB08A2E),
];

// Tones Fork Dark draws that the surfaces above do not name, each sampled in the same survey.

/// A field's placeholder ("Filter", "Commit subject") and a muted line.
pub const PLACEHOLDER: Color = hex(0x999898);
/// The sidebar's chosen entry, Local Changes or All Commits.
pub const SIDEBAR_CHOSEN: Color = hex(0x3F3F3F);
/// The sidebar's icons — a folder, a branch, the check mark, a tag, a stash's box, the gear,
/// Local Changes' and All Commits' — sampled `#CFCFCF` in uw27.
pub const SIDEBAR_ICON: Color = hex(0xCFCFCF);
/// The sidebar's quieter marks: a section's or folder's chevron and a branch with no upstream,
/// sampled `#777777`..`#7B7A7A` in uw27 (a remote's cloud is [`REMOTE_GLYPH`]).
pub const SIDEBAR_DIM_ICON: Color = hex(0x777777);
/// The warning on a branch whose upstream is gone, sampled `#FFC501` (sb14g).
pub const UPSTREAM_GONE: Color = hex(0xFFC501);
/// The sidebar's filter field, sampled `#292929` (FS2 §1).
pub const FILTER_FIELD: Color = hex(0x292929);
/// A banner across the window (Fork's `.gitignore` banner).
pub const BANNER: Color = hex(0x333333);
/// A text field's fill, and an unticked check box's.
pub const FIELD: Color = hex(0x333333);
/// A check box's 1 px border.
pub const CHECK_BOX_BORDER: Color = hex(0x6B6B6B);
/// A link to another commit: a parent's id in the Commit tab.
pub const LINK: Color = hex(0x3E9FF8);
/// An added file's badge.
pub const ADDED_BADGE: Color = hex(0x28CD41);
/// A modified file's badge.
pub const MODIFIED_BADGE: Color = hex(0xFFCC00);
/// A menu's soft drop shadow, ~4 px at its right and bottom, fading to the ground.
pub const MENU_SHADOW: Color = hex(0x1E1E1E);
/// A disabled button's 1 px border.
pub const DISABLED_BORDER: Color = hex(0x383838);
/// The status box's fill, in the toolbar.
pub const STATUS_BOX: Color = hex(0x303030);
/// A count badge on Push's or Pull's icon.
pub const BADGE: Color = hex(0x606060);
/// A count badge's figures: white.
pub const BADGE_TEXT: Color = hex(0xFFFFFF);

/// The dark shade of a lane's colour a branch's or a remote's chip is filled with, as Fork
/// fills its first lane's orange (`#FF9502`) with `#5D3D14`: the colour taken most of the way
/// to black.
pub fn chip_fill(lane: Color) -> Color {
    Color::lerp(lane, Color::BLACK, 0.635)
}

/// The border of the cloud-only chip a branch level with its upstream draws before its own: a
/// darker shade of its lane's colour than the branch's own, as Fork borders it (`~#685727` on
/// its second lane's `#F4C302`).
pub fn upstream_chip_border(lane: Color) -> Color {
    Color::lerp(lane, Color::BLACK, 0.57)
}

/// A selected row's band and text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selected {
    pub band: Color,
    pub text: Color,
}

/// How a selected row is drawn, by whether its list has focus: Fork's blue with white text
/// while it has, its grey with the body's text when not — bold either way.
pub const fn selected(focused: bool) -> Selected {
    if focused {
        Selected {
            band: SELECTION_FOCUSED,
            text: SELECTED_TEXT,
        }
    } else {
        Selected {
            band: SELECTION_UNFOCUSED,
            text: TEXT,
        }
    }
}

/// Freya's dark sheet with Fork Dark's tones in every slot Fork gives one: what Freya's own
/// components resolve their colours against. The accent is Fork's selection blue rather than
/// the desktop's, so a filled button and a ticked check box match the selection; the status
/// slots (success, warning, error, info) and the scrollbar's thumbs stay Freya's.
pub fn sheet() -> ColorsSheet {
    ColorsSheet {
        primary: SELECTION_FOCUSED,
        secondary: Color::lerp(SELECTION_FOCUSED, SELECTED_TEXT, 0.65),
        tertiary: Color::lerp(SELECTION_FOCUSED, Color::BLACK, 0.23),
        background: HISTORY,
        surface_primary: RULE,
        surface_secondary: MENU_HOVER,
        surface_tertiary: BUTTON,
        border: BUTTON_BORDER,
        border_focus: ACCENT,
        text_primary: TEXT,
        text_secondary: DETAIL_LABEL,
        text_placeholder: PLACEHOLDER,
        text_inverse: SELECTED_TEXT,
        text_highlight: ACCENT,
        disabled: DISABLED_TEXT,
        ..DARK_COLORS
    }
}

/// The theme the window launches with: Freya's `dark_theme` over [`sheet`], with the parts of
/// its components the sheet cannot say set to Fork Dark's — a disabled button's fill and
/// border, the menus' ground, border, shadow, hover and square corners, a field's fill and placeholder, the
/// splitters' rule and the dialogs' ground — and the tooltips at the body's size rather than
/// Freya's 14 px.
pub fn theme() -> Theme {
    let mut theme = dark_theme();
    theme.palette = Box::new(sheet());
    let mut button = |key: &'static str, background: Preference<Color>| {
        let colours = ButtonColorsThemePreference {
            background,
            hover_background: Preference::Specific(MENU_HOVER),
            disabled_background: Preference::Specific(DISABLED_FILL),
            border_fill: Preference::Specific(BUTTON_BORDER),
            hover_border_fill: Preference::Specific(BUTTON_BORDER),
            focus_border_fill: Preference::Specific(ACCENT),
            disabled_border_fill: Preference::Specific(DISABLED_BORDER),
            color: Preference::Specific(TEXT),
            hover_color: Preference::Specific(TEXT),
            disabled_color: Preference::Specific(DISABLED_TEXT),
        };
        theme.set(key, colours);
    };
    button("button", Preference::Specific(BUTTON));
    button("outline_button", Preference::Specific(BUTTON));
    theme.set(
        "filled_button",
        ButtonColorsThemePreference {
            background: Preference::Specific(SELECTION_FOCUSED),
            hover_background: Preference::reference("tertiary"),
            disabled_background: Preference::Specific(DISABLED_FILL),
            border_fill: Preference::Specific(SELECTION_FOCUSED),
            hover_border_fill: Preference::reference("tertiary"),
            focus_border_fill: Preference::Specific(ACCENT),
            disabled_border_fill: Preference::Specific(DISABLED_BORDER),
            color: Preference::Specific(SELECTED_TEXT),
            hover_color: Preference::Specific(SELECTED_TEXT),
            disabled_color: Preference::Specific(DISABLED_TEXT),
        },
    );
    theme.set(
        "menu_container",
        MenuContainerThemePreference {
            background: Preference::Specific(MENU),
            padding: Preference::Specific(Gaps::new_all(3.)),
            shadow: Preference::Specific(MENU_SHADOW),
            border_fill: Preference::Specific(MENU_BORDER),
            corner_radius: Preference::Specific(CornerRadius::new_all(0.)),
        },
    );
    theme.set(
        "menu_item",
        MenuItemThemePreference {
            background: Preference::Specific(Color::TRANSPARENT),
            hover_background: Preference::Specific(MENU_HOVER),
            select_background: Preference::Specific(MENU_HOVER),
            border_fill: Preference::Specific(Color::TRANSPARENT),
            focus_border_fill: Preference::Specific(ACCENT),
            corner_radius: Preference::Specific(CornerRadius::new_all(0.)),
            color: Preference::Specific(TEXT),
            select_color: Preference::Specific(SELECTED_TEXT),
        },
    );
    theme.set(
        "tooltip",
        TooltipThemePreference {
            background: Preference::Specific(MENU),
            color: Preference::Specific(TEXT),
            border_fill: Preference::Specific(MENU_BORDER),
            font_family: Preference::Specific(String::new()),
            font_size: Preference::Specific(crate::style::metrics::BODY),
            font_weight: Preference::Specific(400),
        },
    );
    theme.set(
        "input",
        InputColorsThemePreference {
            background: Preference::Specific(FIELD),
            hover_background: Preference::Specific(FIELD),
            focus_background: Preference::Specific(FIELD),
            color: Preference::Specific(TEXT),
            placeholder_color: Preference::Specific(PLACEHOLDER),
            border_fill: Preference::Specific(RULE),
            hover_border_fill: Preference::Specific(RULE),
            focus_border_fill: Preference::Specific(ACCENT),
            focus_ring_fill: Preference::Specific(ACCENT),
        },
    );
    theme.set(
        "resizable_handle",
        ResizableHandleThemePreference {
            background: Preference::Specific(RULE),
            hover_background: Preference::Specific(RULE),
            corner_radius: Preference::Specific(CornerRadius::new_all(0.)),
        },
    );
    if let Some(popup) = theme.get::<PopupThemePreference>("popup").cloned() {
        theme.set(
            "popup",
            PopupThemePreference {
                background: Preference::Specific(DIALOG),
                color: Preference::Specific(TEXT),
                ..popup
            },
        );
    }
    theme
}
