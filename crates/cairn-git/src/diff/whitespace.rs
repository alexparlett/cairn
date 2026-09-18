//! The second, display-only set of changed ranges: what the diff looks like when
//! whitespace is ignored (R2.8).
//!
//! git's `-w` compares lines with all their whitespace removed, and that is what this does.
//! The normalised key is one token per line, so a range over the keys indexes the original
//! lines one for one and nothing has to be mapped back. The lines a view draws are always
//! the original bytes; only which ranges are marked changes.

use cairn_model::ChangedRange;
use gix::diff::blob::{Algorithm, InternedInput};

pub(super) fn changes_ignoring_whitespace(
    old: &[u8],
    new: &[u8],
    algorithm: Algorithm,
) -> Vec<ChangedRange> {
    let old_keys = keys(old);
    let new_keys = keys(new);
    let mut input: InternedInput<&[u8]> = InternedInput::default();
    input.update_before(old_keys.iter().map(Vec::as_slice));
    input.update_after(new_keys.iter().map(Vec::as_slice));
    let diff = gix::diff::blob::diff_with_slider_heuristics(algorithm, &input);
    super::content::changed_ranges(&diff)
}

/// One key per line, in the same order and the same count as the file's own lines.
fn keys(content: &[u8]) -> Vec<Vec<u8>> {
    gix::diff::blob::sources::byte_lines(content)
        .map(|line| {
            line.iter()
                .copied()
                .filter(|byte| !byte.is_ascii_whitespace())
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cairn_model::{LineSpan, split_lines};

    /// The mapping this whole approach rests on: as many keys as lines, on any content.
    #[test]
    fn there_is_exactly_one_key_per_line_however_the_file_ends() {
        for content in [
            &b""[..],
            b"\n",
            b"a\n",
            b"a",
            b"a\nb\nc\n",
            b"a\nb\nc",
            b"\n\n\n",
            b"  \t \n\n   ",
            b"crlf\r\nkept\r\n",
        ] {
            assert_eq!(
                keys(content).len(),
                split_lines(content).len(),
                "{content:?} produced a different number of keys than lines"
            );
        }
    }

    /// The terminator is whitespace too, so a line that lost its newline is not a change
    /// when whitespace is ignored — which is what `git diff -w` reports.
    #[test]
    fn whitespace_only_changes_disappear_and_real_ones_do_not() {
        let changes = changes_ignoring_whitespace(
            b"let x = 1;\n    indented\n",
            b"let  x=1;\n\tindented\n",
            Algorithm::Histogram,
        );
        assert!(
            changes.is_empty(),
            "only the whitespace moved, yet {changes:?} was reported"
        );

        let changes = changes_ignoring_whitespace(b"a\nb\n", b"a\nc\n", Algorithm::Histogram);
        assert_eq!(
            changes,
            vec![ChangedRange::new(LineSpan::at(1, 1), LineSpan::at(1, 1))],
            "a real change must survive ignoring whitespace"
        );

        let changes = changes_ignoring_whitespace(b"a\n", b"a", Algorithm::Histogram);
        assert!(
            changes.is_empty(),
            "a lost final newline is a whitespace change: {changes:?}"
        );
    }

    /// Caught by: dropping blank lines from the key list, which shifts every range after
    /// one and tints the wrong rows.
    #[test]
    fn a_blank_line_keeps_its_place_even_though_its_key_is_empty() {
        let changes =
            changes_ignoring_whitespace(b"a\n\n\nb\n", b"a\n\n\nc\n", Algorithm::Histogram);
        assert_eq!(
            changes,
            vec![ChangedRange::new(LineSpan::at(3, 1), LineSpan::at(3, 1))],
            "the change is on line 4, whatever the blank lines hash to"
        );
    }
}
