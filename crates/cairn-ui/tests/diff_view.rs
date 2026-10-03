//! Headless tests for the unified diff view and the bar over it (PRD R6.2-R6.7, criteria
//! C9 for unified rows and C11).

use std::cell::RefCell;
use std::rc::Rc;

use cairn_model::{
    ByteRange, ChangeStatus, ChangedFile, ChangedRange, Context, DiffContent, DiffLine,
    DisplayOverlay, FileDiff, FileMode, FunctionContext, IntraLineHighlight, LineNumber, LineSpan,
    RepoPath, Similarity, TextDiff, UnifiedRow, UnifiedRows, split_lines,
};
use cairn_ui::diff_palette::{ADDED_EMPHASIS, CURRENT_CHANGE, REMOVED_EMPHASIS};
use cairn_ui::{
    DIFF_ROW_HEIGHT, DiffHeader, DiffSettings, ENTIRE_FILE_LABEL, FEWER_LINES_LABEL,
    HIDDEN_CHANGES_NOTICE, HeaderAction, IGNORE_WHITESPACE_LABEL, MORE_LINES_LABEL,
    NEXT_CHANGE_LABEL, NO_NEWLINE_AT_END, PREVIOUS_CHANGE_LABEL, SIDE_BY_SIDE_LABEL, ShownDiff,
    UnifiedDiffView,
};
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
    let (mut test, _) = TestingRunner::new(
        || {
            let fixture = use_consume::<Fixture>();
            let scroll = use_scroll_controller(ScrollConfig::default);
            rect()
                .expanded()
                .child(UnifiedDiffView::new(fixture.shown, scroll))
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
    let within_a_viewport = |test: &TestingRunner, place: &str| {
        let count = built_rows(test).len();
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

/// One drawn row, read with its colour ignored: the gutter's two numbers, the marker
/// column, the text.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Read {
    old: String,
    new: String,
    marker: String,
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
            let (marker, text) = match rest.as_slice() {
                [text] => (String::new(), text.clone()),
                [marker, text] => (marker.clone(), text.clone()),
                other => panic!("a row of {other:?}"),
            };
            rows.push(Read {
                old,
                new,
                marker,
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

fn read(old: &str, new: &str, marker: &str, text: &str) -> Read {
    Read {
        old: old.to_owned(),
        new: new.to_owned(),
        marker: marker.to_owned(),
        text: text.to_owned(),
    }
}

/// L11, R6.4: a row means the same with its colour ignored — a removed line has an old
/// number, a blank new gutter and `-`; an added one the reverse and `+`; a context line both
/// numbers and a blank marker; a hunk header is git's `@@` line with the function context
/// git printed, and no numbers; git's end-of-file marker is a row of its own. Caught by: a
/// marker dropped, a number drawn on the side a line is not on, a header without its
/// function context.
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
            read("", "", "", "@@ -2,3 +2,3 @@ fn main() {"),
            read("2", "2", " ", "    a();"),
            read("3", "", "-", "    b();"),
            read("", "3", "+", "    c();"),
            read("4", "4", " ", "}"),
            read("", "", "", NO_NEWLINE_AT_END),
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

/// R6.2, R6.3, C11: the bar holds previous and next change, every toggle, and side-by-side
/// disabled, each found by the name assistive technology reads; each enabled button reports
/// its action, fewer lines is disabled at one line and both line buttons while the entire
/// file is shown. Caught by: a button wired to another action, fewer lines pressable at the
/// floor, side-by-side reporting anything.
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
        ],
        "fewer lines at one, or side-by-side, reported a press"
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
    let (test, _) = launch_header(move || DiffHeader::new(file("src/lib.rs"), on));
    let drawn = labels(&test);
    for name in EVERY_BUTTON {
        assert!(
            !drawn.iter().any(|text| text == name),
            "{name:?} is drawn as a caption: {drawn:?}"
        );
        let colours = glyph_colours(&test, name);
        assert!(!colours.is_empty(), "{name:?} draws no glyph");
        if name == IGNORE_WHITESPACE_LABEL || name == ENTIRE_FILE_LABEL {
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
    let width = shown.content_width();
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
