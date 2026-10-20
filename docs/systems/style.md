# The window's look

How Cairn's window gets its sizes, tones and icons today: one module, `cairn_ui::style`
(`crates/cairn-ui/src/style.rs` beside `style/`), from which every render file takes its font
sizes, tones and icons. Layout sizes — row heights, widths, paddings — are still written in the
views, and `style::metrics`' row heights, bars and sidebar width are read as the phases that
lay those regions out land. Intent: `docs/design/ui.md`, "Palette and type". Spec:
`docs/prd/fork-alignment.md` R1.

## One module

- **`style::metrics`** — sizes in logical pixels: Fork's, measured on Fork for Windows at
  1024×768 (`docs/research/ui-review/fork-window-survey-2026-10-16.md`) — the body's 12 px,
  the toolbar labels' 11 px, the caption row's 26 px and the toolbar's 41 px, the rows of the
  history (23), the sidebar (22), Local Changes (20) and a menu (23), the history's graph —
  its 12 px lane pitch, 5 px dot, 13 px merge circle and 2 px line — its 17 px chips, its
  14 px avatars and the 22 px strip over it while Show Lost Commits is on, the sidebar's 300 px and
  its 12 px indent, the diff's 17 px row (the Commit tab's one pitch too), the detail pane's
  strip (31 px between two rules, its tabs 96 px), the Changes tab's 23 px summary and the
  diff's 17 px bar with their rules, the bar's 16 px buttons 20 px apart, the Commit tab's 42 px
  avatar and its 20 px inset, 62 px captions' column and 16 px gap, and the summary's 18 px
  avatar (`fork-window-survey-2-2026-10-16.md` §1b, `fork-committer-2026-10-20.md`) — and the sizes Fork's survey does not give, each
  saying where it comes from (the diff's text and line numbers, measured from Fork earlier;
  a heading's 14 px; the Commit tab's subject and captions, Cairn's).
- **`style::tones`** — Fork Dark's tones, sampled from Fork's dark theme
  (`docs/research/ui-review/fork-window-survey-2-2026-10-16.md`, section 1): every surface,
  text, selection, menu, dialog, button, diff band and chip tone of the PRD's table, then the
  few more the same survey sampled that the table does not name (a field's fill and
  placeholder, a check box's border, a banner, a link, the badges of an added and a modified
  file, a menu's shadow, a disabled button's border, the sidebar's chosen entry, a remote chip's
  grey cloud), the sidebar's sampled in uw27 and the sidebar survey
  (`docs/research/ui-review/fork-sidebar-2026-10-20.md`: its icons, its quieter chevrons, its
  filter's fill and a gone upstream's warning), the diff bar's glyph, its toggled-on blue and
  the square under the pointer (`docs/research/ui-review/fork-committer-2026-10-20.md`), `UNSEEN` for text laid out only to hold its room
  (a menu's copy of a chord), and Cairn's eight avatar colours, the first Fork's sampled lavender
  (`AVATARS`). The lanes keep Cairn's eight
  (`history::lane_palette`); a branch's and a remote's chip is filled with a dark shade of its
  lane (`chip_fill`), and the cloud-only chip of a branch level with its upstream bordered in a
  darker one (`upstream_chip_border`).
- **`style::icons`** — the Lucide icons, compiled in by Freya's `icons-lucide` feature
  (the crate `freya-icons`, from the pinned Freya source; the feature pinned and Lucide's ISC
  licence recorded in `deny.toml`), one function per icon by what it means, drawn at a size
  and in a tone.

`the_style_module_holds_forks_metrics_and_fork_darks_tones` holds every row of the PRD's two
tables to the module's value.

## What reads it

- **Components** name the tone they mean (`tones::TEXT`, `tones::RULE`) and the size
  (`metrics::BODY`). The status tones the table does not name — an error, a warning, a
  renamed file's badge — stay the launch theme's, read from its sheet.
- **The launch theme** (`tones::theme`, installed by `crates/cairn-app/src/main.rs`) is Freya's
  `dark_theme` over a sheet with Fork Dark's tones in every slot Fork gives one (`tones::sheet`:
  the body text, the history's ground, the rules, Fork's blue as the focus border and the
  accent, the selection blue as the primary), with the buttons', menus', fields', tooltips',
  splitters' and dialogs' component themes set where the sheet cannot say Fork's tone; a
  tooltip at the body's size. So Freya's own components draw no stock tone for anything the
  table names (`the_launch_theme_carries_fork_darks_tones_into_freyas_components`).
- **The body size** is set once at the window's root (`crates/cairn-app/src/window.rs`), so a
  text given no size of its own — a menu's, a button's — is 12 px, never Freya's 16 px
  default. Freya's dialog parts set their own (`PopupTitle` 18 px, `PopupContent` 15 px): a
  label inside them is given the body's size where it is built.
- **The diff's palette** (`diff::palette`) reads its ground, bands, changed word, hunk header
  and bar from the tones (`docs/systems/diff.md`, "Colours and typeface").

## The selection

A selected row says whether its list has focus, as Fork's lists do: Fork's blue `#236BD2`
with white bold text while its list has focus, Fork's grey `#525252` with the body's text,
still bold, when not (`tones::selected`). Each list reads its focus from the toolkit
(`use_focus` on its accessibility id) as it renders, hands it to the rows it builds through
the data its virtualizing view compares, so a change of focus draws the visible rows again
and nothing else: the history (`HistoryList`, through `RowRender::focused` to
`CommitRow::focused`), the sidebar (which takes focus when one of its rows is pressed), both of
Local Changes' lists, the Changes tab's files and the Commit tab's file rows. A list takes focus
from a press on one of its rows; the history also as the window opens.
`a_selection_is_blue_while_its_list_has_focus_and_grey_when_not` holds the five lists to it.

## Held by

- `chrome_sizes_and_tones_come_from_the_style_module` (`crates/cairn-guards/tests/invariants.rs`,
  matcher `style_literals`, self-test `the_style_literal_matcher_catches_the_shapes_it_claims`):
  no render file outside the module passes a numeric literal to `font_size` or a constant it
  declares from one, declares a `FONT` constant from one, or builds a colour from a literal —
  a constructor, a named `Color`, a tuple of three integers and an optional alpha, or a `"#…"`
  string — beyond `STYLE_EXCEPTIONS`: `diff/palette.rs` and `history/lane_palette.rs`, which
  excuse a file's tones and never a size, each row failing once its file builds no tone. It
  does not read layout sizes, and the routes a spelling-reader misses (a size through a local
  binding or a parameter, a colour from a `u32` or a variable, a literal alpha) are stated in
  the root `CLAUDE.md`.
- `every_icon_is_named_through_the_icon_module` (matcher `names_an_icon_set`, self-test
  `the_icon_set_matcher_catches_the_shapes_it_claims`): only `style/icons.rs` names the icon
  set, `freya::icons`, `SvgViewer` or Freya's drawn icons.
- `deny.toml`'s `[[bans.features]]` row for `freya`, `exact`: another feature is a decision.
