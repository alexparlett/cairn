//! Headless tests for the diff view, unified and side by side, and the bar over it (PRD
//! R6.1-R6.7, R6.9, criteria C9 and C11).

use std::cell::RefCell;
use std::rc::Rc;

use cairn_model::{
    ByteRange, ChangeStatus, ChangedFile, ChangedRange, Context, DiffContent, DiffLine,
    DisplayOverlay, FileDiff, FileMode, FunctionContext, IntraLineHighlight, LineNumber, LineSpan,
    RepoPath, ShownDiff, Similarity, TextDiff, UnifiedRow, UnifiedRows, split_lines,
};
use cairn_ui::diff_palette::{
    ADDED_EMPHASIS, ADDED_TINT, CURRENT_CHANGE, GUTTER_SEPARATOR, REMOVED_EMPHASIS, REMOVED_TINT,
};
use cairn_ui::{
    DIFF_ROW_HEIGHT, DiffHeader, DiffSettings, DiffView, ENTIRE_FILE_LABEL, FEWER_LINES_LABEL,
    HIDDEN_CHANGES_NOTICE, HeaderAction, IGNORE_WHITESPACE_LABEL, MORE_LINES_LABEL,
    NEXT_CHANGE_LABEL, NO_NEWLINE_AT_END, NUMBER_FONT_SIZE, NUMBER_PADDING, PREVIOUS_CHANGE_LABEL,
    SIDE_BY_SIDE_LABEL, TEXT_PADDING, number_width,
};
use cairn_ui::{SCROLLBAR_THICKNESS, with_end_room};
use freya::prelude::*;
use freya_testing::{TestingNode, TestingRunner};

const WIDTH: f32 = 900.;
const HEIGHT: f32 = 400.;

fn file(path: &str) -> ChangedFile {
    ChangedFile {
        status: ChangeStatus::Modified,
        old_path: RepoPath::from(path),
        new_path: RepoPath::from(path),
        old_mode: Some(FileMode::Regular),
        new_mode: Some(FileMode::Regular),
        old_id: None,
        new_id: None,
    }
}

fn change(removed: (u32, u32), added: (u32, u32)) -> ChangedRange {
    ChangedRange::new(
        LineSpan::at(removed.0, removed.1),
        LineSpan::at(added.0, added.1),
    )
}

fn text_diff(text: TextDiff, overlay: DisplayOverlay) -> FileDiff {
    FileDiff {
        file: file("src/lib.rs"),
        content: DiffContent::Text { text, overlay },
    }
}

/// `lines` lines, `line {n}`, with every fiftieth replaced by `LINE {n}` on the new side.
fn long_file(lines: u32) -> FileDiff {
    let old: Vec<DiffLine> = (0..lines)
        .map(|n| DiffLine::terminated(format!("line {n}")))
        .collect();
    let new: Vec<DiffLine> = (0..lines)
        .map(|n| {
            DiffLine::terminated(if n % 50 == 25 {
                format!("LINE {n}")
            } else {
                format!("line {n}")
            })
        })
        .collect();
    let changes = (0..lines / 50)
        .map(|k| change((k * 50 + 25, 1), (k * 50 + 25, 1)))
        .collect();
    text_diff(TextDiff::new(old, new, changes), DisplayOverlay::none())
}

#[derive(Clone)]
struct Fixture {
    shown: State<ShownDiff>,
}

fn launch(shown: ShownDiff) -> TestingRunner {
    launch_as(shown, false)
}

fn launch_side_by_side(shown: ShownDiff) -> TestingRunner {
    launch_as(shown, true)
}

fn launch_as(shown: ShownDiff, side_by_side: bool) -> TestingRunner {
    let (mut test, _) = TestingRunner::new(
        move || {
            let fixture = use_consume::<Fixture>();
            let scroll = use_scroll_controller(ScrollConfig::default);
            rect()
                .expanded()
                .child(DiffView::new(fixture.shown, scroll).side_by_side(side_by_side))
                .into_element()
        },
        (WIDTH, HEIGHT).into(),
        move |runner| {
            runner.provide_root_context(move || Fixture {
                shown: State::create(shown),
            })
        },
        1.,
    );
    test.sync_and_update();
    test
}

/// Every row's text paragraph — each row draws exactly one — as `(text, visible)`.
fn built_rows(test: &TestingRunner) -> Vec<(String, bool)> {
    test.find_many(|node, element| {
        Paragraph::try_downcast(element).map(|paragraph| {
            let text: String = paragraph
                .spans
                .iter()
                .map(|span| span.text.as_ref())
                .collect();
            (text, node.is_visible())
        })
    })
}

/// C9 for unified rows, the twin of `only_a_viewport_of_rows_is_built_however_long_the_history`:
/// over a 1,000-line and a 100,000-line file drawn whole, the view builds one viewport of
/// rows at the top, scrolled deep and scrolled to the end — the same number at each — and
/// the end is the projection's last row, so the view's length is the projection's. Caught
/// by: a plain list building every row, a length that drops or adds rows (the last row
/// never reached, or an empty one past it), or rows built for the whole scrolled distance.
#[test]
fn only_a_viewport_of_diff_rows_is_built_however_long_the_file() {
    let viewport_rows = (HEIGHT / DIFF_ROW_HEIGHT).ceil() as usize;
    // Scrolled to the end, the empty rows after the last (`end_room`) are built and draw no
    // text: counted here, so the end builds what the top does.
    let room = with_end_room(1, DIFF_ROW_HEIGHT) - 1;
    let within_a_viewport = |test: &TestingRunner, place: &str| {
        let count = built_rows(test).len()
            + if place.starts_with("at the end") {
                room
            } else {
                0
            };
        assert!(
            count >= viewport_rows && count <= viewport_rows + 2,
            "{count} rows were built for a {viewport_rows}-row viewport {place}"
        );
        count
    };

    let mut built = Vec::new();
    for lines in [1_000u32, 100_000] {
        let diff = long_file(lines);
        let shown = ShownDiff::new(diff.clone(), Context::EntireFile);
        let (DiffContent::Text { text, overlay }, _) = (&diff.content, ()) else {
            unreachable!("a text diff")
        };
        let projection = UnifiedRows::shown(text, overlay, Context::EntireFile);
        // Counted from the file, not from a second projection: the entire file is one
        // header and every line once, plus the added side of each of its one-line changes.
        let changes = (lines / 50) as usize;
        assert_eq!(
            shown.row_count(),
            lines as usize + changes + 1,
            "the entire file is not one header and every line"
        );
        assert_eq!(projection.len(), shown.row_count());
        let last_text = match projection.row(projection.len() - 1) {
            Some(UnifiedRow::Context { line, .. }) => line.text().into_owned(),
            other => panic!("the last row of an entire file is a context line: {other:?}"),
        };

        let mut test = launch(shown);
        built.push(within_a_viewport(&test, &format!("at the top of {lines}")));
        assert!(
            built_rows(&test)
                .iter()
                .any(|(text, visible)| *visible && text.starts_with("@@ -1,")),
            "the top row is not the hunk's header"
        );

        let deep = projection.len() * 3 / 5;
        test.scroll(
            (100., 100.),
            (0., -(deep as f64 * f64::from(DIFF_ROW_HEIGHT))),
        );
        built.push(within_a_viewport(
            &test,
            &format!("at row {deep} of {lines}"),
        ));
        let deep_line = match projection.row(deep) {
            Some(
                UnifiedRow::Context { line, .. }
                | UnifiedRow::Removed { line, .. }
                | UnifiedRow::Added { line, .. },
            ) => line.text().into_owned(),
            other => panic!("row {deep} is a line: {other:?}"),
        };
        assert!(
            built_rows(&test)
                .iter()
                .any(|(text, visible)| *visible && *text == deep_line),
            "scrolling to row {deep} did not build it"
        );

        test.scroll(
            (100., 100.),
            (
                0.,
                -(projection.len() as f64 * f64::from(DIFF_ROW_HEIGHT) * 2.),
            ),
        );
        built.push(within_a_viewport(&test, &format!("at the end of {lines}")));
        let rows = built_rows(&test);
        assert!(
            rows.iter()
                .any(|(text, visible)| *visible && *text == last_text),
            "the end of the view is not the projection's last row, {last_text:?}: {:?}",
            rows.iter().rev().take(3).collect::<Vec<_>>()
        );
    }
    // At the top, deep and at the end, the longer file built what the shorter one did. (At
    // the end the last row is flush with the bottom, so no row is cut in half there: one
    // fewer than where the view stops between rows.)
    let (short, long) = built.split_at(3);
    assert_eq!(
        short, long,
        "a longer file built a different number of rows at the same place: {built:?}"
    );
    assert!(
        built
            .iter()
            .max()
            .zip(built.iter().min())
            .is_some_and(|(most, least)| most - least <= 1),
        "a deeper scroll built more rows: {built:?}"
    );
}

/// The user's report (2026-10-07): scrolled to the end, the last row sits above the horizontal
/// scrollbar, which Freya draws over the viewport's bottom, never under it — unified and side
/// by side, over a file whose lines are wider than the view, so the bar is there to cover it.
/// Caught by: the list exactly as long as its rows (the last row flush with the bottom edge).
#[test]
fn scrolled_to_the_end_the_last_row_is_clear_of_the_horizontal_scrollbar() {
    let lines = 200u32;
    let wide = "x".repeat(400);
    let old: Vec<DiffLine> = (0..lines)
        .map(|n| DiffLine::terminated(format!("line {n} {wide}")))
        .collect();
    let mut new = old.clone();
    new[100] = DiffLine::terminated(format!("LINE 100 {wide}"));
    let diff = text_diff(
        TextDiff::new(old, new, vec![change((100, 1), (100, 1))]),
        DisplayOverlay::none(),
    );
    for side_by_side in [false, true] {
        let mut test = launch_as(
            ShownDiff::new(diff.clone(), Context::EntireFile),
            side_by_side,
        );
        test.scroll(
            (100., 100.),
            (0., -(f64::from(lines) * f64::from(DIFF_ROW_HEIGHT) * 4.)),
        );
        let last = format!("line {} {wide}", lines - 1);
        let bottom = test
            .find_many(|node, element| {
                Paragraph::try_downcast(element).and_then(|paragraph| {
                    let text: String = paragraph
                        .spans
                        .iter()
                        .map(|span| span.text.as_ref())
                        .collect();
                    (node.is_visible() && text == last).then(|| node.layout().area.max_y())
                })
            })
            .into_iter()
            .fold(f32::MIN, f32::max);
        assert!(
            bottom > 0.,
            "the last line is not drawn at the end (side by side: {side_by_side})"
        );
        assert!(
            bottom <= HEIGHT - SCROLLBAR_THICKNESS,
            "the last row ends at {bottom}, under the horizontal scrollbar over the bottom \
             {SCROLLBAR_THICKNESS} px of the {HEIGHT} px view (side by side: {side_by_side})"
        );
    }
}

/// One drawn row, read with its colour ignored: the gutter's two numbers and the text —
/// and nothing else, since no marker column is drawn (the user's decision, 2026-10-04).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Read {
    old: String,
    new: String,
    text: String,
}

/// The rows on screen, top to bottom, as text alone. Each row's labels and paragraph share
/// its top edge.
fn read_rows(test: &TestingRunner) -> Vec<Read> {
    let mut pieces: Vec<(i32, f32, String)> = test.find_many(|node, element| {
        let area = node.layout().area;
        let at = (area.min_y() / DIFF_ROW_HEIGHT).floor() as i32;
        if let Some(label) = Label::try_downcast(element) {
            return Some((at, area.min_x(), label.text.to_string()));
        }
        Paragraph::try_downcast(element).map(|paragraph| {
            let text: String = paragraph
                .spans
                .iter()
                .map(|span| span.text.as_ref())
                .collect();
            (at, area.min_x(), format!("¶{text}"))
        })
    });
    pieces.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
    let mut rows: Vec<Read> = Vec::new();
    let mut current: Option<(i32, Vec<String>)> = None;
    for (at, _, text) in pieces.into_iter().chain([(i32::MAX, 0., String::new())]) {
        if current.as_ref().is_some_and(|(row, _)| *row != at) {
            let (_, parts) = current.take().unwrap_or_default();
            let mut parts = parts.into_iter();
            let old = parts.next().unwrap_or_default();
            let new = parts.next().unwrap_or_default();
            let rest: Vec<String> = parts.collect();
            let text = match rest.as_slice() {
                [text] if text.starts_with('¶') => text.clone(),
                other => panic!("a row of {other:?} after its numbers, not its text alone"),
            };
            rows.push(Read {
                old,
                new,
                text: text.trim_start_matches('¶').to_owned(),
            });
        }
        if at == i32::MAX {
            break;
        }
        current.get_or_insert_with(|| (at, Vec::new())).1.push(text);
    }
    rows
}

fn read(old: &str, new: &str, text: &str) -> Read {
    Read {
        old: old.to_owned(),
        new: new.to_owned(),
        text: text.to_owned(),
    }
}

/// R6.4, and the user's decision of 2026-10-04 (no marker column, as Fork's default): a
/// row draws its numbers and its text and nothing between them, and the gutter alone still
/// says what the tint says — a removed line has an old number and a blank new gutter, an
/// added one the reverse, a context line both; a hunk header is git's `@@` line with the
/// function context git printed, and no numbers; git's end-of-file marker is a row of its
/// own. Caught by: a `-`/`+` column drawn again, a number drawn on the side a line is not
/// on, a header without its function context.
#[test]
fn a_row_reads_the_same_with_its_colour_ignored() {
    let text = TextDiff::new(
        split_lines(b"fn main() {\n    a();\n    b();\n}"),
        split_lines(b"fn main() {\n    a();\n    c();\n}"),
        vec![change((2, 1), (2, 1))],
    );
    let overlay = DisplayOverlay::none().with_function_context(FunctionContext::read_at(
        Context::lines(1),
        vec![(LineNumber::from_index(1), b"fn main() {".to_vec())],
    ));
    let test = launch(ShownDiff::new(text_diff(text, overlay), Context::lines(1)));
    assert_eq!(
        read_rows(&test),
        [
            read("", "", "@@ -2,3 +2,3 @@ fn main() {"),
            read("2", "2", "    a();"),
            read("3", "", "    b();"),
            read("", "3", "    c();"),
            read("4", "4", "}"),
            read("", "", NO_NEWLINE_AT_END),
        ]
    );
}

/// A row's text, its highlighted UTF-16 ranges, and the colour they are drawn in.
type Highlighted = (String, Vec<(usize, usize)>, Color);

fn highlighted(test: &TestingRunner) -> Vec<Highlighted> {
    test.find_many(|_, element| {
        Paragraph::try_downcast(element).map(|paragraph| {
            let text: String = paragraph
                .spans
                .iter()
                .map(|span| span.text.as_ref())
                .collect();
            (
                text,
                paragraph.highlights.clone(),
                paragraph.cursor_style_data.highlight_color,
            )
        })
    })
}

/// C11: intra-line ranges are drawn — in the stronger tint of their line's side, at the
/// columns they cover after a tab is expanded — and a context line, or an added line with
/// no pair, has none. The paired lines sit at different numbers on their sides and carry
/// different ranges, so each side's ranges are looked up by its own line. Caught by:
/// dropping the ranges, swapping the two sides' tints, passing byte offsets through, or
/// looking a removed line up among the added lines' pairs (or the other way about).
#[test]
fn intra_line_ranges_are_drawn_in_the_stronger_tint() {
    let text = TextDiff::new(
        split_lines(b"same\n\tlet x = 1;\n"),
        split_lines(b"same\nfresh\n\tlet yy = 1;\n"),
        vec![change((1, 1), (1, 2))],
    );
    let overlay = DisplayOverlay::new(
        None,
        vec![IntraLineHighlight {
            removed_line: LineNumber::from_index(1),
            added_line: LineNumber::from_index(2),
            on_removed: vec![ByteRange::new(5, 6)],
            on_added: vec![ByteRange::new(5, 7)],
        }],
    );
    let test = launch(ShownDiff::new(text_diff(text, overlay), Context::lines(3)));
    let rows = highlighted(&test);
    let find = |wanted: &str| {
        rows.iter()
            .find(|(text, _, _)| text == wanted)
            .unwrap_or_else(|| panic!("no row reads {wanted:?}: {rows:?}"))
    };
    let removed = find("        let x = 1;");
    assert_eq!(removed.1, [(12, 13)]);
    assert_eq!(removed.2, REMOVED_EMPHASIS);
    let added = find("        let yy = 1;");
    assert_eq!(added.1, [(12, 14)]);
    assert_eq!(added.2, ADDED_EMPHASIS);
    assert!(
        find("fresh").1.is_empty(),
        "an unpaired added line was highlighted"
    );
    assert!(find("same").1.is_empty(), "a context line was highlighted");
}

/// R6.7, C11: ignoring whitespace hides whitespace-only changes — no row is drawn for them —
/// and the answer says a change is hidden only when one is: not when whitespace is not
/// ignored, and not when ignoring it hides nothing. Caught by: the notice following the
/// toggle rather than the answer, or rows drawn from the exact ranges.
#[test]
fn ignoring_whitespace_hides_whitespace_only_changes_and_says_so_only_then() {
    let respaced = || {
        TextDiff::new(
            split_lines(b"a\n  b\nc\n"),
            split_lines(b"a\nb\nc\n"),
            vec![change((1, 1), (1, 1))],
        )
    };
    let hidden = ShownDiff::new(
        text_diff(
            respaced(),
            DisplayOverlay::new(Some(Vec::new()), Vec::new()),
        ),
        Context::lines(3),
    );
    assert!(hidden.hides_changes());
    assert_eq!(hidden.row_count(), 0, "a whitespace-only change was drawn");
    let test = launch(hidden);
    assert!(built_rows(&test).is_empty());

    let exact = ShownDiff::new(
        text_diff(respaced(), DisplayOverlay::none()),
        Context::lines(3),
    );
    assert!(!exact.hides_changes(), "whitespace is not ignored");
    assert!(exact.row_count() > 0);

    let real = TextDiff::new(
        split_lines(b"a\nb\nc\n"),
        split_lines(b"a\nB\nc\n"),
        vec![change((1, 1), (1, 1))],
    );
    let nothing_hidden = ShownDiff::new(
        text_diff(
            real,
            DisplayOverlay::new(Some(vec![change((1, 1), (1, 1))]), Vec::new()),
        ),
        Context::lines(3),
    );
    assert!(!nothing_hidden.hides_changes(), "ignoring hid nothing here");
}

/// Every button of the bar, left to right, by name.
const EVERY_BUTTON: [&str; 7] = [
    PREVIOUS_CHANGE_LABEL,
    NEXT_CHANGE_LABEL,
    IGNORE_WHITESPACE_LABEL,
    FEWER_LINES_LABEL,
    MORE_LINES_LABEL,
    ENTIRE_FILE_LABEL,
    SIDE_BY_SIDE_LABEL,
];

fn launch_header(
    header: impl Fn() -> DiffHeader + 'static,
) -> (TestingRunner, Rc<RefCell<Vec<HeaderAction>>>) {
    let pressed: Rc<RefCell<Vec<HeaderAction>>> = Rc::default();
    let reported = pressed.clone();
    let (mut test, _) = TestingRunner::new(
        move || {
            use_init_theme(dark_theme);
            let reported = reported.clone();
            rect()
                .expanded()
                .child(header().on_action(move |action| reported.borrow_mut().push(action)))
                .into_element()
        },
        (WIDTH, HEIGHT).into(),
        |_| {},
        1.,
    );
    test.sync_and_update();
    (test, pressed)
}

fn labels(test: &TestingRunner) -> Vec<String> {
    test.find_many(|_, element| Label::try_downcast(element).map(|l| l.text.to_string()))
}

/// The value a rect hands its button as the button's name, if it hands one.
macro_rules! named {
    ($element:expr) => {
        Rect::try_downcast($element)
            .and_then(|rect| rect.accessibility.builder.value().map(str::to_owned))
    };
}

/// Every name the bar's buttons carry, in order.
fn names(test: &TestingRunner) -> Vec<String> {
    test.find_many(|_, element| named!(element))
}

/// Presses the button named `name` — by what assistive technology reads, not by a glyph.
fn click(test: &mut TestingRunner, name: &str) {
    let centre = test
        .find(|node, element| {
            named!(element)
                .filter(|value| value == name)
                .map(|_| node.layout().area.center())
        })
        .unwrap_or_else(|| panic!("no button is named {name:?}: {:?}", names(test)));
    test.click_cursor((f64::from(centre.x), f64::from(centre.y)));
    test.sync_and_update();
}

/// Every colour the glyph of the button named `name` is drawn in: its shapes' fills and
/// borders, and its characters.
fn glyph_colours(test: &TestingRunner, name: &str) -> Vec<Color> {
    fn walk(node: TestingNode, into: &mut Vec<Color>) {
        let element = node.element();
        if let Some(rect) = Rect::try_downcast(element.as_ref()) {
            into.extend(rect.style.background.as_color());
            into.extend(rect.style.borders.iter().map(|border| border.fill));
        }
        if let Some(label) = Label::try_downcast(element.as_ref()) {
            into.extend(label.text_style_data.color.and_then(|fill| fill.as_color()));
        }
        for child in node.children() {
            walk(child, into);
        }
    }
    let glyph = test
        .find(|node, element| named!(element).filter(|value| value == name).map(|_| node))
        .unwrap_or_else(|| panic!("no button is named {name:?}: {:?}", names(test)));
    let mut colours = Vec::new();
    walk(glyph, &mut colours);
    colours.retain(|colour| *colour != Color::TRANSPARENT);
    colours
}

/// R6.2, R6.3, C11: the bar holds previous and next change and every toggle, side-by-side
/// among them (phase 07), each found by the name assistive technology reads; each enabled
/// button reports its action, fewer lines is disabled at one line and both line buttons while
/// the entire file is shown. Caught by: a button wired to another action, fewer lines
/// pressable at the floor, side-by-side still disabled.
#[test]
fn the_bar_reports_each_button_and_holds_fewer_lines_at_one() {
    let mut at_one = DiffSettings::default();
    at_one.fewer_lines();
    at_one.fewer_lines();
    let (mut test, pressed) = launch_header(move || DiffHeader::new(file("src/lib.rs"), at_one));
    assert_eq!(
        names(&test),
        EVERY_BUTTON,
        "the bar's buttons, in Fork's order"
    );
    for name in EVERY_BUTTON {
        click(&mut test, name);
    }
    assert_eq!(
        pressed.borrow().as_slice(),
        [
            HeaderAction::PreviousChange,
            HeaderAction::NextChange,
            HeaderAction::IgnoreWhitespace,
            HeaderAction::MoreLines,
            HeaderAction::EntireFile,
            HeaderAction::SideBySide,
        ],
        "fewer lines at one reported a press, or side-by-side did not"
    );

    let mut entire = DiffSettings::default();
    entire.toggle_entire_file();
    let (mut test, pressed) = launch_header(move || DiffHeader::new(file("src/lib.rs"), entire));
    click(&mut test, FEWER_LINES_LABEL);
    click(&mut test, MORE_LINES_LABEL);
    click(&mut test, ENTIRE_FILE_LABEL);
    assert_eq!(pressed.borrow().as_slice(), [HeaderAction::EntireFile]);
}

/// The backgrounds of the bar's buttons, left to right.
fn button_backgrounds(test: &TestingRunner) -> Vec<Option<Color>> {
    test.find_many(|_, element| {
        Rect::try_downcast(element)
            .filter(|rect| rect.accessibility.builder.role() == AccessibilityRole::Button)
            .map(|rect| rect.style.background.as_color())
    })
}

/// The user's decision (2026-10-03), Fork's bar: every button is a glyph whose name — Fork's
/// tooltip where one is recorded — is read by assistive technology rather than drawn as a
/// caption; a toggle that is on has its glyph drawn wholly in the accent (the theme's
/// `text_highlight`, which `CURRENT_CHANGE` copies) and is not filled, and a toggle that is
/// off, or a plain button, draws none of it. Caught by: a caption drawn in place of a glyph,
/// a button with no name, an active toggle filled, or its glyph not lit.
#[test]
fn every_button_is_a_named_glyph_and_an_active_toggle_is_lit_in_the_accent() {
    let mut on = DiffSettings::default();
    on.toggle_ignore_whitespace();
    on.toggle_entire_file();
    on.toggle_side_by_side();
    let (test, _) = launch_header(move || DiffHeader::new(file("src/lib.rs"), on));
    let drawn = labels(&test);
    for name in EVERY_BUTTON {
        assert!(
            !drawn.iter().any(|text| text == name),
            "{name:?} is drawn as a caption: {drawn:?}"
        );
        let colours = glyph_colours(&test, name);
        assert!(!colours.is_empty(), "{name:?} draws no glyph");
        if name == IGNORE_WHITESPACE_LABEL
            || name == ENTIRE_FILE_LABEL
            || name == SIDE_BY_SIDE_LABEL
        {
            assert!(
                colours.iter().all(|colour| *colour == CURRENT_CHANGE),
                "{name:?} is on and its glyph is not the accent: {colours:?}"
            );
        } else {
            assert!(
                !colours.contains(&CURRENT_CHANGE),
                "{name:?} is not on and its glyph is lit: {colours:?}"
            );
        }
    }
    let backgrounds = button_backgrounds(&test);
    assert_eq!(backgrounds.len(), EVERY_BUTTON.len(), "{backgrounds:?}");
    assert_eq!(
        backgrounds.get(2),
        backgrounds.get(4),
        "an active toggle is filled: {backgrounds:?}"
    );

    let (test, _) =
        launch_header(move || DiffHeader::new(file("src/lib.rs"), DiffSettings::default()));
    for name in EVERY_BUTTON {
        assert!(
            !glyph_colours(&test, name).contains(&CURRENT_CHANGE),
            "{name:?} is lit with nothing on"
        );
    }
}

/// R6.7: the notice is drawn when the answer hides a change and not otherwise, whatever the
/// toggle says; the path is drawn as its directory and its emphasised name. Caught by: a
/// notice shown whenever whitespace is ignored.
#[test]
fn the_bar_says_changes_are_hidden_only_when_they_are() {
    let mut ignoring = DiffSettings::default();
    ignoring.toggle_ignore_whitespace();
    let (test, _) = launch_header(move || DiffHeader::new(file("src/lib.rs"), ignoring));
    assert!(!labels(&test).iter().any(|l| l == HIDDEN_CHANGES_NOTICE));
    let spans: Vec<String> = test.find_many(|_, element| {
        Paragraph::try_downcast(element).map(|paragraph| {
            paragraph
                .spans
                .iter()
                .map(|span| span.text.to_string())
                .collect::<Vec<_>>()
                .join("|")
        })
    });
    assert_eq!(spans, ["src/|lib.rs"]);

    let (test, _) =
        launch_header(move || DiffHeader::new(file("src/lib.rs"), ignoring).hiding(true));
    assert!(labels(&test).iter().any(|l| l == HIDDEN_CHANGES_NOTICE));

    let mut renamed = file("new/name.rs");
    renamed.old_path = RepoPath::from("old/name.rs");
    renamed.status = ChangeStatus::Renamed(Similarity::from_percent(90));
    let (test, _) =
        launch_header(move || DiffHeader::new(renamed.clone(), DiffSettings::default()));
    assert!(
        test.find(|_, element| Paragraph::try_downcast(element))
            .is_some(),
        "a renamed file's path is drawn"
    );
}

/// The QA brief's horizontal extent: every row is as wide as the answer's widest line,
/// measured once, so the extent is the same whether that line is built or not — at the top,
/// where it is not, and at the end, where it is. Caught by: rows sized to their own text
/// (the horizontal scrollbar would change as the view scrolls).
#[test]
fn the_horizontal_extent_is_the_widest_lines_wherever_the_view_is() {
    let lines = 600u32;
    let old: Vec<DiffLine> = (0..lines)
        .map(|n| {
            DiffLine::terminated(if n == lines - 1 {
                "w".repeat(400)
            } else {
                format!("line {n}")
            })
        })
        .collect();
    let mut new = old.clone();
    new[1] = DiffLine::terminated("LINE 1");
    let shown = ShownDiff::new(
        text_diff(
            TextDiff::new(old, new, vec![change((1, 1), (1, 1))]),
            DisplayOverlay::none(),
        ),
        Context::EntireFile,
    );
    let width = cairn_ui::content_width(&shown);
    assert!(
        width > WIDTH,
        "the fixture's widest line is narrower than the view"
    );
    // Each row is the widest rect of its row's height at its top edge; its parts are inside.
    let row_widths = |test: &TestingRunner| -> Vec<f32> {
        let mut rects: Vec<(i32, f32)> = test.find_many(|node, element| {
            let area = node.layout().area;
            Rect::try_downcast(element)
                .filter(|_| area.height() == DIFF_ROW_HEIGHT)
                .map(|_| (area.min_y().round() as i32, area.width()))
        });
        rects.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.total_cmp(&a.1)));
        rects.dedup_by_key(|(top, _)| *top);
        rects.into_iter().map(|(_, width)| width).collect()
    };

    let mut test = launch(shown);
    assert!(
        !built_rows(&test)
            .iter()
            .any(|(text, _)| text.starts_with("wwww")),
        "the widest line is built at the top, so the test decides nothing"
    );
    let top = row_widths(&test);
    assert!(!top.is_empty());
    assert!(top.iter().all(|w| *w == width), "{top:?} against {width}");

    test.scroll(
        (100., 100.),
        (0., -f64::from(lines) * f64::from(DIFF_ROW_HEIGHT) * 2.),
    );
    assert!(
        built_rows(&test)
            .iter()
            .any(|(text, _)| text.starts_with("wwww"))
    );
    let end = row_widths(&test);
    assert!(end.iter().all(|w| *w == width), "{end:?} against {width}");
}

/// The rows a side-by-side view built: one per distinct top edge among its row-high rects.
fn built_side_rows(test: &TestingRunner) -> usize {
    let mut tops: Vec<i32> = test.find_many(|node, element| {
        let area = node.layout().area;
        Rect::try_downcast(element)
            .filter(|_| area.height() == DIFF_ROW_HEIGHT)
            .map(|_| area.min_y().round() as i32)
    });
    tops.sort_unstable();
    tops.dedup();
    tops.len()
}

/// Every paragraph drawn, with whether it is visible and which column it is in.
fn side_paragraphs(test: &TestingRunner) -> Vec<(String, bool, bool)> {
    test.find_many(|node, element| {
        Paragraph::try_downcast(element).map(|paragraph| {
            let text: String = paragraph
                .spans
                .iter()
                .map(|span| span.text.as_ref())
                .collect();
            (
                text,
                node.is_visible(),
                node.layout().area.min_x() < WIDTH / 2.,
            )
        })
    })
}

/// C9 for side-by-side rows, the twin of `only_a_viewport_of_diff_rows_is_built_however_long_
/// the_file`: over a 1,000-line and a 100,000-line file drawn whole and side by side, the view
/// builds one viewport of rows at the top, scrolled deep and scrolled to the end — the same
/// number at each — its length is the file's real row count (one header and one row per line,
/// a replaced line and its replacement sharing one), counted from the file and not from a
/// second projection, and the end is the last line, in both columns. Caught by: a plain
/// list, two scroll views, a length that drops or adds a row, or pairing that builds a row
/// per changed line.
#[test]
fn only_a_viewport_of_side_by_side_rows_is_built_however_long_the_file() {
    let viewport_rows = (HEIGHT / DIFF_ROW_HEIGHT).ceil() as usize;
    // Scrolled to the end, the empty rows after the last (`end_room`) are built and draw no
    // text: counted here, so the end builds what the top does.
    let room = with_end_room(1, DIFF_ROW_HEIGHT) - 1;
    let within_a_viewport = |test: &TestingRunner, place: &str| {
        let count = built_side_rows(test)
            + if place.starts_with("at the end") {
                room
            } else {
                0
            };
        assert!(
            count >= viewport_rows && count <= viewport_rows + 2,
            "{count} rows were built for a {viewport_rows}-row viewport {place}"
        );
        count
    };
    let mut built = Vec::new();
    for lines in [1_000u32, 100_000] {
        let shown = ShownDiff::new(long_file(lines), Context::EntireFile);
        assert_eq!(
            shown.rows(true),
            lines as usize + 1,
            "side by side, the entire file is not one header and a row per line"
        );
        let mut test = launch_side_by_side(shown);
        built.push(within_a_viewport(&test, &format!("at the top of {lines}")));
        let top = side_paragraphs(&test);
        assert!(
            top.iter()
                .any(|(text, visible, left)| *visible && *left && text.starts_with("@@ -1,"))
                && top
                    .iter()
                    .any(|(text, visible, left)| *visible && !*left && text.starts_with("@@ -1,")),
            "the hunk's header is not at the top of each column: {top:?}"
        );

        let deep = lines as usize * 3 / 5;
        test.scroll(
            (100., 100.),
            (0., -(deep as f64 * f64::from(DIFF_ROW_HEIGHT))),
        );
        built.push(within_a_viewport(
            &test,
            &format!("at row {deep} of {lines}"),
        ));
        let line = format!("line {}", deep - 1);
        assert!(
            side_paragraphs(&test)
                .iter()
                .any(|(text, visible, _)| *visible && *text == line),
            "scrolling to row {deep} did not build {line:?}"
        );

        test.scroll(
            (100., 100.),
            (0., -(f64::from(lines) * f64::from(DIFF_ROW_HEIGHT) * 2.)),
        );
        built.push(within_a_viewport(&test, &format!("at the end of {lines}")));
        let last = format!("line {}", lines - 1);
        let end = side_paragraphs(&test);
        for left in [true, false] {
            assert!(
                end.iter()
                    .any(|(text, visible, column)| *visible && *column == left && *text == last),
                "the end of the view is not the last line in the {} column: {:?}",
                if left { "left" } else { "right" },
                end.iter().rev().take(4).collect::<Vec<_>>()
            );
        }
    }
    let (short, long) = built.split_at(3);
    assert_eq!(
        short, long,
        "a longer file built a different number of rows: {built:?}"
    );
}

/// One side-by-side row read with its colour ignored: each column's number and text, or
/// `None` for a column that is filler.
type SideRow = (Option<(String, String)>, Option<(String, String)>);

/// The side-by-side rows on screen, top to bottom, each column read apart. A column with no
/// text (only its blank number) is filler.
fn read_side_rows(test: &TestingRunner) -> Vec<SideRow> {
    let mut pieces: Vec<(i32, f32, String)> = test.find_many(|node, element| {
        let area = node.layout().area;
        let at = (area.min_y() / DIFF_ROW_HEIGHT).floor() as i32;
        if let Some(label) = Label::try_downcast(element) {
            return Some((at, area.min_x(), label.text.to_string()));
        }
        Paragraph::try_downcast(element).map(|paragraph| {
            let text: String = paragraph
                .spans
                .iter()
                .map(|span| span.text.as_ref())
                .collect();
            (at, area.min_x(), format!("¶{text}"))
        })
    });
    pieces.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
    let mut rows: Vec<SideRow> = Vec::new();
    let mut at = None;
    let mut columns: [Vec<String>; 2] = [Vec::new(), Vec::new()];
    let read = |parts: &mut Vec<String>| {
        let parts = std::mem::take(parts);
        if !parts.iter().any(|part| part.starts_with('¶')) {
            assert!(
                parts.iter().all(String::is_empty),
                "a filler column drew {parts:?}"
            );
            return None;
        }
        let number = parts.first().cloned().unwrap_or_default();
        let text = match &parts[1..] {
            [text] if text.starts_with('¶') => text.clone(),
            other => panic!("a column of {other:?} after its number, not its text alone"),
        };
        Some((number, text.trim_start_matches('¶').to_owned()))
    };
    for (row, x, text) in pieces.into_iter().chain([(i32::MAX, 0., String::new())]) {
        if at.is_some_and(|current| current != row) {
            let [left, right] = &mut columns;
            rows.push((read(left), read(right)));
        }
        if row == i32::MAX {
            break;
        }
        at = Some(row);
        columns[usize::from(x >= WIDTH / 2.)].push(text);
    }
    rows
}

fn cell(number: &str, text: &str) -> Option<(String, String)> {
    Some((number.to_owned(), text.to_owned()))
}

/// R1.4, R6.1, R6.4 side by side: the i-th removed line beside the i-th added one, the
/// shorter side's rows filler, one number per column and no marker (the user's decision of
/// 2026-10-04, as Fork's default), the hunk header with git's function context in both
/// columns, a context line in both, and git's end-of-file marker in the column of the side
/// whose last line did not end. Here the sides differ in length both ways, and a whole side
/// of one change is empty. Caught by: pairing by position in the hunk rather than in the
/// change, filler drawn as a blank line with a number, the header in one column only, or a
/// `-`/`+` drawn again.
#[test]
fn side_by_side_pairs_lines_fills_the_shorter_side_and_reads_without_colour() {
    let text = TextDiff::new(
        split_lines(b"fn f() {\n    a();\n    b();\n}\nx\ny"),
        split_lines(b"fn f() {\n    A();\n}\nnew\nx\nY"),
        vec![
            change((1, 2), (1, 1)),
            change((4, 0), (3, 1)),
            change((5, 1), (5, 1)),
        ],
    );
    let overlay = DisplayOverlay::none().with_function_context(FunctionContext::read_at(
        Context::lines(9),
        vec![(LineNumber::from_index(0), b"fn f() {".to_vec())],
    ));
    let test = launch_side_by_side(ShownDiff::new(text_diff(text, overlay), Context::lines(9)));
    let header = "@@ -1,6 +1,6 @@ fn f() {";
    assert_eq!(
        read_side_rows(&test),
        [
            (cell("", header), cell("", header)),
            (cell("1", "fn f() {"), cell("1", "fn f() {")),
            (cell("2", "    a();"), cell("2", "    A();")),
            (cell("3", "    b();"), None),
            (cell("4", "}"), cell("3", "}")),
            (None, cell("4", "new")),
            (cell("5", "x"), cell("5", "x")),
            (cell("6", "y"), cell("6", "Y")),
            (cell("", NO_NEWLINE_AT_END), cell("", NO_NEWLINE_AT_END)),
        ]
    );
}

/// R6.1, Fork's equal panes (Finding 11): the two columns are equal and together fill the
/// view whatever the lines' lengths, each with its own gutter; a sideways scroll slides both
/// columns' text by the same distance while each gutter stays put, and the view's extent is
/// the widest line's overflow past its column. Caught by: columns sized to their text (one
/// side pushed off screen), a gutter that scrolls away, or the sides scrolled apart.
#[test]
fn side_by_side_columns_are_equal_halves_and_slide_together() {
    let wide = "w".repeat(300);
    let text = TextDiff::new(
        split_lines(format!("a\n{wide}\nc\n").as_bytes()),
        split_lines(format!("a\n{wide}!\nc\n").as_bytes()),
        vec![change((1, 1), (1, 1))],
    );
    let mut test = launch_side_by_side(ShownDiff::new(
        text_diff(text, DisplayOverlay::none()),
        Context::lines(3),
    ));
    // Each column: the widest row-high rect whose top is the changed row's, left and right.
    let column_widths = |test: &TestingRunner| -> Vec<(f32, f32)> {
        let mut columns: Vec<(f32, f32)> = test.find_many(|node, element| {
            let area = node.layout().area;
            Rect::try_downcast(element)
                .filter(|_| {
                    area.height() == DIFF_ROW_HEIGHT
                        && (area.min_y() - 2. * DIFF_ROW_HEIGHT).abs() < 0.5
                        && area.width() > WIDTH * 0.49
                        && area.width() < WIDTH * 0.55
                })
                .map(|_| (area.min_x(), area.width()))
        });
        columns.sort_by(|a, b| a.0.total_cmp(&b.0));
        columns.dedup();
        columns
    };
    let columns = column_widths(&test);
    assert_eq!(columns.len(), 2, "{columns:?}");
    assert!((columns[0].1 - columns[1].1).abs() < 0.5, "{columns:?}");
    assert!(
        columns[0].1 * 2. <= WIDTH && columns[0].1 * 2. > WIDTH - 4.,
        "{columns:?}"
    );

    let positions = |test: &TestingRunner| -> (Vec<f32>, Vec<f32>) {
        let numbers = test.find_many(|node, element| {
            Label::try_downcast(element)
                .filter(|label| label.text == "2")
                .map(|_| node.layout().area.min_x())
        });
        let texts = test.find_many(|node, element| {
            Paragraph::try_downcast(element)
                .filter(|paragraph| {
                    paragraph
                        .spans
                        .first()
                        .is_some_and(|span| span.text.starts_with("www"))
                })
                .map(|_| node.layout().area.min_x())
        });
        (numbers, texts)
    };
    let (numbers, texts) = positions(&test);
    assert_eq!(
        (numbers.len(), texts.len()),
        (2, 2),
        "{numbers:?} {texts:?}"
    );
    test.scroll((WIDTH as f64 / 2., 40.), (-200., 0.));
    test.sync_and_update();
    let (moved_numbers, moved_texts) = positions(&test);
    assert_eq!(moved_numbers, numbers, "a gutter scrolled away");
    let slid: Vec<f32> = texts.iter().zip(&moved_texts).map(|(a, b)| a - b).collect();
    assert!(slid[0] > 0. && (slid[0] - slid[1]).abs() < 0.5, "{slid:?}");
}

/// R6.9, phase 06 QA's obligation: a file loaded past the limits with one line of several
/// mebibytes draws that line cut at the long-line limit with the marker after it, in either
/// view, so a row costs at most the limit's text however long the line, and the horizontal
/// extent is the cut's, not the line's; the marker says how many bytes are not drawn. Caught
/// by: a row that draws the whole line, a cut with no marker, a marker that miscounts, or an
/// extent measured from the line's length.
#[test]
fn a_line_past_the_limit_is_drawn_cut_with_its_marker_in_both_views() {
    let long = "x".repeat(4 * 1024 * 1024);
    let text = TextDiff::new(
        vec![DiffLine::terminated("short")],
        vec![DiffLine::terminated(long)],
        vec![change((0, 1), (0, 1))],
    );
    let shown = ShownDiff::new(text_diff(text, DisplayOverlay::none()), Context::lines(3));
    let cut = cairn_model::LINE_CUT_BYTES;
    let bound = 2. * 20. + 200. + (cut + cairn_ui::cut_marker(64 << 20).len()) as f32 * 7.;
    assert!(
        cairn_ui::content_width(&shown) < bound,
        "the extent is {} for a line drawn to {cut} bytes",
        cairn_ui::content_width(&shown)
    );
    for side_by_side in [false, true] {
        let test = launch_as(shown.clone(), side_by_side);
        let drawn: Vec<Vec<String>> = test.find_many(|_, element| {
            Paragraph::try_downcast(element).map(|paragraph| {
                paragraph
                    .spans
                    .iter()
                    .map(|span| span.text.to_string())
                    .collect()
            })
        });
        let line = drawn
            .iter()
            .find(|spans| spans.first().is_some_and(|text| text.starts_with("xxx")))
            .unwrap_or_else(|| panic!("the long line is not drawn: {drawn:?}"));
        assert_eq!(line.len(), 2, "no marker after the cut: {:?}", line.len());
        assert_eq!(line[0].len(), cut, "side by side: {side_by_side}");
        // The user's marker (2026-10-03): how many of the line's bytes are not drawn.
        assert_eq!(line[1], " … 4,192,256 more bytes");
        assert_eq!(4 * 1024 * 1024 - cut, 4_192_256);
    }
}

/// Twelve lines, the seventh changed: two-digit numbers, a removed and an added row.
fn two_digit_change() -> ShownDiff {
    let old: String = (1..=12).map(|n| format!("line {n}\n")).collect();
    let new = old.replace("line 7\n", "LINE 7\n");
    let text = TextDiff::new(
        split_lines(old.as_bytes()),
        split_lines(new.as_bytes()),
        vec![change((6, 1), (6, 1))],
    );
    ShownDiff::new(text_diff(text, DisplayOverlay::none()), Context::lines(3))
}

/// The one-pixel separators between a gutter and its text on screen, each `(row, area)`.
/// In side-by-side the rule between the columns is one too; callers take the nearest.
fn separators(test: &TestingRunner) -> Vec<(i32, Area)> {
    test.find_many(|node, element| {
        let area = node.layout().area;
        Rect::try_downcast(element)
            .filter(|rect| {
                area.width() == 1.
                    && area.height() == DIFF_ROW_HEIGHT
                    && rect.style.background.as_color() == Some(GUTTER_SEPARATOR)
            })
            .map(|_| ((area.min_y() / DIFF_ROW_HEIGHT).floor() as i32, area))
    })
}

/// The line numbers on screen, each `(row, area, font size)`.
fn number_labels(test: &TestingRunner) -> Vec<(i32, Area, Option<f32>)> {
    test.find_many(|node, element| {
        let area = node.layout().area;
        Label::try_downcast(element)
            .filter(|label| {
                !label.text.is_empty() && label.text.chars().all(|c| c.is_ascii_digit())
            })
            .map(|label| {
                (
                    (area.min_y() / DIFF_ROW_HEIGHT).floor() as i32,
                    area,
                    label.text_style_data.font_size.map(f32::from),
                )
            })
    })
}

/// The line texts on screen, each `(row, area)`.
fn line_paragraphs(test: &TestingRunner) -> Vec<(i32, Area)> {
    test.find_many(|node, element| {
        let area = node.layout().area;
        Paragraph::try_downcast(element)
            .map(|_| ((area.min_y() / DIFF_ROW_HEIGHT).floor() as i32, area))
    })
}

/// The user's visual check (2026-10-04): "there is no gap between the line numbers and the
/// divider", "the gutter column is too big for the text size", and Fork's captures
/// (user-supplied, 2026-10-04): small numbers, a few pixels either side of the divider, no
/// marker column. In both views every number's box ends at least `NUMBER_PADDING` before
/// the separator to its right, every line's text starts at least `TEXT_PADDING` after the
/// separator to its left, the numbers are drawn at `NUMBER_FONT_SIZE`, smaller than the
/// text, and the unified gutter is two columns of `number_width` for the file's widest
/// number. Caught by: the padding set on the number's own label — which this toolkit
/// build lays its paragraph out over, right-aligning the digits against the separator —
/// the numbers drawn at the text's size, or a marker column between separator and text.
#[test]
fn the_gutter_is_small_and_leaves_a_gap_either_side_of_its_separator_in_both_views() {
    for side_by_side in [false, true] {
        let test = launch_as(two_digit_change(), side_by_side);
        let separators = separators(&test);
        let numbers = number_labels(&test);
        let texts = line_paragraphs(&test);
        assert!(
            numbers.len() >= 10 && texts.len() >= 8,
            "too little drawn to decide anything: {} numbers, {} texts",
            numbers.len(),
            texts.len()
        );
        for (row, number, size) in &numbers {
            assert_eq!(*size, Some(NUMBER_FONT_SIZE), "a number's size");
            let separator = separators
                .iter()
                .filter(|(at, area)| at == row && area.min_x() >= number.max_x() - 0.01)
                .map(|(_, area)| area.min_x())
                .reduce(f32::min)
                .unwrap_or_else(|| panic!("no separator right of a number on row {row}"));
            assert!(
                number.max_x() <= separator - NUMBER_PADDING + 0.01,
                "side by side {side_by_side}: a number ends {} px before its separator, not \
                 {NUMBER_PADDING}",
                separator - number.max_x()
            );
        }
        for (row, text) in &texts {
            let separator = separators
                .iter()
                .filter(|(at, area)| at == row && area.max_x() <= text.min_x() + 0.01)
                .map(|(_, area)| area.max_x())
                .reduce(f32::max)
                .unwrap_or_else(|| panic!("no separator left of a text on row {row}"));
            assert!(
                text.min_x() >= separator + TEXT_PADDING - 0.01,
                "side by side {side_by_side}: a text starts {} px after its separator, not \
                 {TEXT_PADDING}",
                text.min_x() - separator
            );
        }
        if !side_by_side {
            let gutter = separators
                .iter()
                .map(|(_, area)| area.min_x())
                .reduce(f32::min)
                .unwrap_or_default();
            assert!(
                (gutter - 2. * number_width(2)).abs() < 0.01,
                "the unified gutter is {gutter} px, not two columns of two digits"
            );
        }
    }
    const { assert!(NUMBER_FONT_SIZE < cairn_ui::diff_palette::DIFF_FONT_SIZE) };
}

/// Fork's captures (user-supplied, 2026-10-04): in unified the gutter keeps the plain
/// ground on a changed row and the tint starts at the separator; side by side the tint
/// runs across the pane, its number column included. Read as the backgrounds of the rects
/// that hold a removed and an added row's number; and in both views the tint is what is
/// drawn behind the changed line's text — the innermost rect with a background under the
/// text's first glyph is the row's tint. Caught by: the side-by-side tint left after the
/// number, the unified gutter tinted, or the tint held to the number column or covered by
/// another ground before it reaches the text.
#[test]
fn a_changed_rows_tint_starts_at_the_separator_in_unified_and_spans_the_number_side_by_side() {
    for side_by_side in [false, true] {
        let test = launch_as(two_digit_change(), side_by_side);
        let tinted: Vec<(Area, Color)> = test.find_many(|node, element| {
            let area = node.layout().area;
            Rect::try_downcast(element).and_then(|rect| {
                rect.style
                    .background
                    .as_color()
                    .filter(|colour| [REMOVED_TINT, ADDED_TINT].contains(colour))
                    .map(|colour| (area, colour))
            })
        });
        let separators = separators(&test);
        let numbers = number_labels(&test);
        let grounds: Vec<(Area, Color)> = test.find_many(|node, element| {
            let area = node.layout().area;
            Rect::try_downcast(element).and_then(|rect| {
                rect.style
                    .background
                    .as_color()
                    .filter(|colour| *colour != Color::TRANSPARENT)
                    .map(|colour| (area, colour))
            })
        });
        let mut reached = Vec::new();
        for (row, text) in line_paragraphs(&test) {
            let (x, y) = (text.min_x() + 1., text.min_y() + text.height() / 2.);
            let innermost = grounds
                .iter()
                .filter(|(area, _)| {
                    area.min_x() <= x && x <= area.max_x() && area.min_y() <= y && y <= area.max_y()
                })
                .min_by(|(a, _), (b, _)| {
                    (a.width() * a.height()).total_cmp(&(b.width() * b.height()))
                })
                .map(|(_, colour)| *colour);
            if let Some(colour @ (REMOVED_TINT | ADDED_TINT)) = innermost {
                reached.push((row, colour));
            }
        }
        for tint in [REMOVED_TINT, ADDED_TINT] {
            assert!(
                reached.iter().any(|(_, colour)| *colour == tint),
                "side by side {side_by_side}: no changed line's text is drawn on {tint:?}"
            );
        }
        for (number, tint) in [("7", REMOVED_TINT), ("7", ADDED_TINT)] {
            let rows: Vec<&(Area, Color)> = tinted
                .iter()
                .filter(|(_, colour)| *colour == tint)
                .collect();
            assert!(!rows.is_empty(), "no row is tinted {tint:?}");
            for (area, _) in rows {
                let row = (area.min_y() / DIFF_ROW_HEIGHT).floor() as i32;
                let covers_a_number = numbers.iter().any(|(at, label, _)| {
                    *at == row && label.min_x() >= area.min_x() && label.max_x() <= area.max_x()
                });
                assert_eq!(
                    covers_a_number, side_by_side,
                    "side by side {side_by_side}: line {number}'s tint covers its number: \
                     {covers_a_number}"
                );
                if !side_by_side {
                    let separator = separators
                        .iter()
                        .filter(|(at, _)| *at == row)
                        .map(|(_, sep)| sep.max_x())
                        .next()
                        .unwrap_or_else(|| panic!("no separator on row {row}"));
                    assert!(
                        (area.min_x() - separator).abs() < 0.01,
                        "the unified tint starts at {}, the separator ends at {separator}",
                        area.min_x()
                    );
                }
            }
        }
    }
}
