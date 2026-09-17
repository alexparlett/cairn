//! What the model promises about selections and patches, over the awkward cases.
//!
//! Criterion C4 of `docs/prd/diff-engine.md` lives here:
//! `a_selection_and_its_patch_are_the_same_in_every_view`.
//!
//! The round-trip tests judge the emitter against two other paths written from different
//! statements of the same rule: the reference applier, which reads the patch format, and
//! `diffs::expected_result`, which reads the selection straight off the changed ranges with
//! no hunks, headers, context or counts involved. Three ways of saying it, one answer.

mod diffs;

use cairn_model::{
    Context, Hunks, LineNumber, PATCH_CONTEXT, Patch, Selection, SideBySideRow, SideBySideRows,
    TextDiff, UnifiedRow, UnifiedRows, apply_patch, apply_patch_in_reverse, emit_patch,
};

use diffs::{Fixture, corpus, expected_lines, expected_result, is_a_well_formed_file};

/// Every context a view can be in, plus the two it opens and closes at.
fn every_context() -> Vec<Context> {
    vec![
        Context::lines(1),
        Context::lines(2),
        Context::lines(3),
        Context::lines(5),
        Context::lines(40),
        Context::EntireFile,
    ]
}

/// The changed lines a unified view offers to click, in the order it draws them.
fn unified_identities(text: &TextDiff, context: Context) -> Vec<(bool, u32)> {
    let rows = UnifiedRows::new(text, context);
    (0..rows.len())
        .filter_map(|row| match rows.row(row) {
            Some(UnifiedRow::Removed { old, .. }) => Some((true, old.index())),
            Some(UnifiedRow::Added { new, .. }) => Some((false, new.index())),
            Some(UnifiedRow::Header(_)) | Some(UnifiedRow::Context { .. }) | None => None,
        })
        .collect()
}

/// The same, from a side-by-side view, which draws the very same lines in another order and
/// another shape.
fn side_by_side_identities(text: &TextDiff, context: Context) -> Vec<(bool, u32)> {
    let rows = SideBySideRows::new(text, context);
    let mut identities = Vec::new();
    for row in 0..rows.len() {
        match rows.row(row) {
            Some(SideBySideRow::Replaced { old, new, .. }) => {
                identities.push((true, old.index()));
                identities.push((false, new.index()));
            }
            Some(SideBySideRow::Removed { old, .. }) => identities.push((true, old.index())),
            Some(SideBySideRow::Added { new, .. }) => identities.push((false, new.index())),
            Some(SideBySideRow::Header(_)) | Some(SideBySideRow::Context { .. }) | None => {}
        }
    }
    identities
}

fn selection_of(identities: &[(bool, u32)]) -> Selection {
    let mut selection = Selection::empty();
    for (removed, line) in identities {
        if *removed {
            selection.select_removed(LineNumber::from_index(*line));
        } else {
            selection.select_added(LineNumber::from_index(*line));
        }
    }
    selection
}

/// Every other changed line, taken from the diff itself rather than from any view — so the
/// selection under test was made without one.
fn every_other_change(text: &TextDiff) -> Selection {
    let mut selection = Selection::empty();
    for (at, (removed, line)) in diffs::every_identity(text).into_iter().enumerate() {
        if at % 2 == 0 {
            if removed {
                selection.select_removed(LineNumber::from_index(line));
            } else {
                selection.select_added(LineNumber::from_index(line));
            }
        }
    }
    selection
}

/// **PRD criterion C4.** A selection survives unified rows, side-by-side rows, every
/// context size and entire-file mode unchanged, and the emitter's output depends on none of
/// them.
///
/// What it decides, in three parts. Every changed line is offered by every projection at
/// every context, so clicking them all in any one view builds the identical `Selection`.
/// The identities a row carries are the line's own numbers, not positions within a hunk or
/// a viewport, so the two views agree line for line. And the patch built from one selection
/// is the same bytes whatever was on screen when it was made — which it has to be, since
/// the emitter is handed no view and no context at all.
///
/// Caught by: numbering rows from the hunk; a projection that leaves a changed line out at
/// some context; an emitter that read a context from anywhere but its own constant.
#[test]
fn a_selection_and_its_patch_are_the_same_in_every_view() {
    for fixture in corpus() {
        let Fixture { name, file, text } = fixture;
        let every_change = Selection::with_every_change(&text);
        let partial = every_other_change(&text);
        let mut from_all_views: Vec<Selection> = Vec::new();

        for context in every_context() {
            let unified = unified_identities(&text, context);
            let side = side_by_side_identities(&text, context);

            let mut unified_sorted = unified.clone();
            let mut side_sorted = side.clone();
            unified_sorted.sort_unstable();
            side_sorted.sort_unstable();
            let mut from_the_diff = diffs::every_identity(&text);
            from_the_diff.sort_unstable();

            assert_eq!(
                unified_sorted, from_the_diff,
                "{name} at {context:?}: the unified view did not offer every changed line"
            );
            assert_eq!(
                side_sorted, from_the_diff,
                "{name} at {context:?}: the side-by-side view did not offer every changed line"
            );

            from_all_views.push(selection_of(&unified));
            from_all_views.push(selection_of(&side));
        }

        for made_in_a_view in &from_all_views {
            assert_eq!(
                made_in_a_view, &every_change,
                "{name}: clicking every changed line in a view did not give the whole selection"
            );
        }

        // The patch of one selection, built once for every view and context there is.
        let mut patches: Vec<Patch> = Vec::new();
        for selection in [&every_change, &partial, &Selection::empty()] {
            let first = emit_patch(&file, &text, selection);
            for context in every_context() {
                let unified = UnifiedRows::new(&text, context);
                let side = SideBySideRows::new(&text, context);
                // A side-by-side view pairs a change's removed and added lines where a
                // unified view lists them, so its row count is smaller exactly when some
                // change has lines on both sides. Caught by: pairing with `min` or with a
                // sum, either of which loses or doubles a row.
                let pairs = text
                    .changes()
                    .iter()
                    .any(|change| !change.removed.is_empty() && !change.added.is_empty());
                assert!(
                    unified.len() >= side.len(),
                    "{name} at {context:?}: pairing grew"
                );
                assert_eq!(
                    unified.len() > side.len(),
                    pairs,
                    "{name} at {context:?}: the two views' row counts do not follow the pairing"
                );
                assert_eq!(
                    emit_patch(&file, &text, selection),
                    first,
                    "{name} at {context:?}: the patch changed with what was on screen"
                );
            }
            patches.push(first);
        }
        assert_eq!(patches.len(), 3);
    }
}

/// C4's other half, stated independently. Asserting `emit_patch(x) == emit_patch(x)` cannot
/// fail — the emitter is pure and no view can reach its arguments — so the claim that the
/// patch does not follow the view needs a construction that does not come from the emitter:
/// these exact bytes, and a context count taken from `PATCH_CONTEXT` rather than from
/// whatever the views were built at.
///
/// Caught by: a changed `PATCH_CONTEXT`, a recount, a header, or an emitter that read a
/// context from anywhere.
#[test]
fn the_patch_of_a_known_selection_is_these_exact_bytes() {
    let old = b"a\nb\nc\nd\ne\nf\ng\nh\ni\n";
    let new = b"a\nb\nc\nd\nE\nf\ng\nh\ni\n";
    let text = diffs::text_of(old, new);
    let file = Fixture::modified("one edit with context either side", old, new).file;
    let patch = emit_patch(&file, &text, &Selection::with_every_change(&text));

    assert_eq!(
        patch.text(),
        "diff --git a/f.txt b/f.txt\n\
         --- a/f.txt\n\
         +++ b/f.txt\n\
         @@ -2,7 +2,7 @@\n\
         \x20b\n\x20c\n\x20d\n\
         -e\n\
         +E\n\
         \x20f\n\x20g\n\x20h\n"
    );

    let context_lines = patch
        .text()
        .lines()
        .filter(|line| line.starts_with(' '))
        .count();
    assert_eq!(
        context_lines,
        2 * PATCH_CONTEXT as usize,
        "the patch did not carry PATCH_CONTEXT lines either side of its change"
    );
}

/// The shape each fixture is supposed to have, pinning the fixture module's own line
/// differ. The round trips cannot see it: `diffs::expected_result` reads the same changed
/// ranges the emitter reads, so a coarser but still valid edit script would agree with
/// itself everywhere and quietly stop exercising hunk grouping.
///
/// `(fixture name, changed ranges, hunks at a context of three)`.
const EXPECTED_SHAPES: &[(&str, usize, usize)] = &[
    ("one line in the middle", 1, 1),
    ("a hunk at the first line", 1, 1),
    ("a hunk at the last line", 1, 1),
    ("a one line file", 1, 1),
    ("an empty file gaining content", 1, 1),
    ("a file losing all its content", 1, 1),
    ("an old side with no final newline", 1, 1),
    ("a new side with no final newline", 1, 1),
    ("neither side with a final newline", 1, 1),
    ("a last line edited where neither side ends", 1, 1),
    ("CRLF content", 1, 1),
    ("CRLF content with no final newline", 1, 1),
    ("blank lines around a change", 1, 1),
    ("a blank line added", 1, 1),
    ("a blank line removed", 1, 1),
    ("adjacent hunks four lines apart", 2, 1),
    ("hunks far enough apart to separate", 2, 2),
    ("an insertion and a removal in one file", 2, 2),
    ("more lines added than removed", 1, 1),
    ("more lines removed than added", 1, 1),
    ("a new file", 1, 1),
    ("a new file that never ends", 1, 1),
    ("a new empty file", 0, 0),
    ("a deleted file", 1, 1),
    ("a deleted empty file", 0, 0),
    ("a rename with no edit", 0, 0),
    ("a rename with edits", 1, 1),
    ("a mode change with no edit", 0, 0),
    ("a mode change with an edit", 1, 1),
];

#[test]
fn every_fixture_has_the_shape_it_was_written_to_have() {
    let corpus = corpus();
    assert_eq!(
        corpus.len(),
        EXPECTED_SHAPES.len(),
        "a fixture was added or removed without its expected shape"
    );
    for fixture in corpus {
        let Fixture { name, text, .. } = fixture;
        let Some((_, changes, hunks)) = EXPECTED_SHAPES.iter().find(|(known, _, _)| *known == name)
        else {
            panic!("the fixture {name:?} has no expected shape");
        };
        assert_eq!(
            text.changes().len(),
            *changes,
            "{name}: the fixture differ produced a different number of changed ranges"
        );
        assert_eq!(
            Hunks::of(&text, Context::lines(3)).len(),
            *hunks,
            "{name}: the fixture no longer groups into the hunks it was written for"
        );
    }
}

/// A patch applied to what it was made from gives exactly what the selection means, for
/// every seeded selection over every awkward case the format allows.
///
/// Caught by: a recount that counts the diff's lines rather than the emitted ones; a hunk
/// header whose new side forgot an earlier hunk; a `\ No newline` marker left on the wrong
/// line or dropped with the line it belonged to.
#[test]
fn a_patch_gives_what_the_selection_means_over_every_awkward_case() {
    for fixture in corpus()
        .into_iter()
        .chain(corpus().into_iter().map(Fixture::with_ids))
    {
        let Fixture { name, file, text } = fixture;
        let mut selections = vec![
            Selection::empty(),
            Selection::with_every_change(&text),
            every_other_change(&text),
        ];
        selections.extend((1..=16u64).map(|seed| diffs::seeded_selection(&text, seed)));

        for (at, selection) in selections.iter().enumerate() {
            let patch = emit_patch(&file, &text, selection);
            let applied = apply_patch(&text.old_content(), patch.as_bytes()).unwrap_or_else(|e| {
                panic!("{name}, selection {at}: the patch did not apply: {e}\n{patch:?}")
            });
            assert_eq!(
                applied,
                expected_result(&text, selection),
                "{name}, selection {at}: the patch did not give what the selection means\n{patch:?}"
            );
        }
    }
}

/// Selecting everything reproduces the new version exactly — the whole-file stage, which is
/// what C1 will put through real git.
#[test]
fn selecting_every_change_reproduces_the_new_version() {
    for fixture in corpus() {
        let Fixture { name, file, text } = fixture;
        let patch = emit_patch(&file, &text, &Selection::with_every_change(&text));
        let applied = apply_patch(&text.old_content(), patch.as_bytes())
            .unwrap_or_else(|e| panic!("{name}: the patch did not apply: {e}\n{patch:?}"));
        assert_eq!(
            applied,
            text.new_content(),
            "{name}: the whole selection did not reproduce the new version\n{patch:?}"
        );
    }
}

/// The same patch backwards takes the result back to what it was made from (R1.6).
///
/// Skipped for a selection whose result is not a file a patch could be taken of again: a
/// kept last line that never ended, with lines added after it, leaves the run-on line in the
/// middle. That is what `git apply` makes of the same patch, so it is the format's answer
/// and not the emitter's, but the reverse of it has nothing well-formed to work on.
#[test]
fn the_same_patch_backwards_undoes_exactly_what_it_did() {
    for fixture in corpus() {
        let Fixture { name, file, text } = fixture;
        let mut selections = vec![
            Selection::with_every_change(&text),
            every_other_change(&text),
        ];
        selections.extend((1..=16u64).map(|seed| diffs::seeded_selection(&text, seed)));

        for (at, selection) in selections.iter().enumerate() {
            if !is_a_well_formed_file(&expected_lines(&text, selection)) {
                continue;
            }
            let patch = emit_patch(&file, &text, selection);
            let forward = apply_patch(&text.old_content(), patch.as_bytes())
                .unwrap_or_else(|e| panic!("{name}, selection {at}: {e}\n{patch:?}"));
            let back = apply_patch_in_reverse(&forward, patch.as_bytes()).unwrap_or_else(|e| {
                panic!("{name}, selection {at}: the patch would not reverse: {e}\n{patch:?}")
            });
            assert_eq!(
                back,
                text.old_content(),
                "{name}, selection {at}: reversing the patch did not restore the old version\n{patch:?}"
            );
        }
    }
}

/// The oracle has to be able to fail, or the round-trip above proves nothing. A patch built
/// for one selection must not give what another selection means.
#[test]
fn a_patch_for_one_selection_does_not_give_what_another_one_means() {
    let text = diffs::text_of(b"a\nb\nc\nd\ne\n", b"a\nB\nc\nD\ne\n");
    let file = Fixture::modified("two edits", b"a\nb\nc\nd\ne\n", b"a\nB\nc\nD\ne\n").file;

    let whole = Selection::with_every_change(&text);
    let mut half = Selection::empty();
    half.select_removed(LineNumber::from_index(1));
    half.select_added(LineNumber::from_index(1));

    let patch = emit_patch(&file, &text, &whole);
    let applied = apply_patch(&text.old_content(), patch.as_bytes()).expect("the patch applies");
    assert_ne!(
        applied,
        expected_result(&text, &half),
        "the whole selection's patch gave what half of it means, so the comparison decides nothing"
    );
    assert_eq!(applied, expected_result(&text, &whole));
}

/// The patch a projection could have leaked into: an emitter reading the view's context
/// would write a different number of context lines at every setting.
#[test]
fn a_patch_is_the_same_bytes_whatever_the_view_was_built_at() {
    let old = b"a\nb\nc\nd\ne\nf\ng\nh\ni\nj\nk\nl\nm\nn\no\np\n";
    let new = b"a\nb\nc\nd\ne\nF\ng\nh\ni\nj\nk\nl\nm\nn\no\nP\n";
    let text = diffs::text_of(old, new);
    let file = Fixture::modified("two far-apart edits", old, new).file;
    let selection = Selection::with_every_change(&text);
    let expected = emit_patch(&file, &text, &selection);

    for context in every_context() {
        let rows = UnifiedRows::new(&text, context);
        let drawn: Vec<String> = (0..rows.len())
            .map(|row| format!("{:?}", rows.row(row)))
            .collect();
        assert!(!drawn.is_empty(), "{context:?} drew nothing");
        assert_eq!(
            emit_patch(&file, &text, &selection),
            expected,
            "the patch followed the view's context of {context:?}"
        );
    }

    let three = emit_patch(&file, &text, &selection);
    assert!(
        three.text().contains("@@ -3,7 +3,7 @@"),
        "the first hunk did not carry three lines of context: {three:?}"
    );
    assert!(
        three.text().contains("@@ -13,4 +13,4 @@"),
        "the second hunk did not stop at the end of the file: {three:?}"
    );
}
