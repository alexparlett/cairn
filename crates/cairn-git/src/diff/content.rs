//! One file's content, both versions, and what gix says differs between them.
//!
//! Every read goes through gix's resource cache in `Mode::ToGit`, which is the mode that
//! never runs a textconv program and the form `git diff` compares and `git apply --cached`
//! expects (R2.3). The cache is also built with
//! `skip_internal_diff_if_external_is_configured` off, so a `diff.<driver>.command` in the
//! user's config is read and never run.

use cairn_model::{
    ChangedFile, ChangedRange, DiffContent, DiffLimits, DisplayOverlay, FileDiff, FileMode,
    LineSpan, Oid, SizeLimit, TextDiff, split_lines,
};
use gix::diff::blob::{Algorithm, InternedInput, ResourceKind, platform::prepare_diff::Operation};
use gix::objs::tree::EntryKind;

use crate::object_id::object_id;
use crate::{Error, Repository};

use super::ContentOptions;

/// A Git LFS pointer names its own version first; the format caps a pointer at 1 KiB.
const LFS_PREFIX: &[u8] = b"version https://git-lfs.github.com/spec/";
const LFS_MAX_BYTES: usize = 1024;

pub(super) fn file_diff(
    repo: &Repository,
    cache: &mut gix::diff::blob::Platform,
    file: &ChangedFile,
    options: &ContentOptions,
) -> Result<FileDiff, Error> {
    Ok(FileDiff {
        file: file.clone(),
        content: content_of(repo, cache, file, options)?,
    })
}

fn content_of(
    repo: &Repository,
    cache: &mut gix::diff::blob::Platform,
    file: &ChangedFile,
    options: &ContentOptions,
) -> Result<DiffContent, Error> {
    // A submodule is a commit id in a tree, not a blob; gix's blob platform refuses the
    // mode outright, so this is decided before anything is read.
    if file.old_mode == Some(FileMode::Submodule) || file.new_mode == Some(FileMode::Submodule) {
        return Ok(DiffContent::Submodule {
            old_commit: file.old_id,
            new_commit: file.new_id,
        });
    }

    // Same blob on both sides and a different mode: the whole change is the mode, and
    // reading the content would only prove it is identical. On a commit that renames
    // 27,592 files this is the difference between a file list and inflating every blob.
    if let (Some(old), Some(new)) = (file.old_id, file.new_id)
        && old == new
        && file.mode_changed()
    {
        return Ok(DiffContent::ModeChangeOnly);
    }

    let inner = repo.inner();
    let ceiling = if options.load_anyway {
        options.limits.load_anyway_bytes
    } else {
        options.limits.max_bytes
    };

    // R2.6: the size limit is tested BEFORE the content is read. An object's header
    // carries its size, so a file too large to draw is refused without inflating it.
    for id in [file.old_id, file.new_id].into_iter().flatten() {
        let size = blob_size(inner, &id, file)?;
        if size > ceiling {
            return Ok(DiffContent::TooLarge {
                crossed: SizeLimit::Bytes {
                    limit: ceiling,
                    measured: size,
                },
                loadable: !options.load_anyway && size <= options.limits.load_anyway_bytes,
            });
        }
    }

    // Keyed by id for an object from the database, so a file already read in this session
    // is not read again; cleared per file so the cache cannot grow with the query.
    cache.clear_resource_cache_keep_allocation();
    set_side(inner, cache, file, ResourceKind::OldOrSource)?;
    set_side(inner, cache, file, ResourceKind::NewOrDestination)?;

    let prepared = cache.prepare_diff().map_err(|source| Error::DiffFile {
        path: file.new_path.to_string(),
        source: Box::new(source),
    })?;
    let algorithm = match prepared.operation {
        // git's own rule: the `diff`/`binary` attribute, `core.bigFileThreshold`, or a NUL
        // byte in the first 8,000 bytes (R2.5). gix clears the buffer of a binary resource,
        // so the size is all there is to show.
        Operation::SourceOrDestinationIsBinary => {
            return Ok(DiffContent::Binary {
                old_size: byte_size(prepared.old.data),
                new_size: byte_size(prepared.new.data),
            });
        }
        // Unreachable while the cache is built with `skip_internal_diff_if_external_is_configured`
        // off, which is what `gix::diff::resource_cache` does and what R2.3 requires: the
        // configured program is read and never started. Answered rather than asserted, so a
        // gix that changed that default draws a notice instead of running a program.
        Operation::ExternalCommand { command } => {
            return Ok(DiffContent::Unsupported {
                reason: format!(
                    "an external diff program is configured for this path ({command}), and Cairn \
                     never runs one"
                ),
            });
        }
        Operation::InternalDiff { algorithm } => algorithm,
    };

    let old_bytes = prepared.old.data.as_slice().unwrap_or_default();
    let new_bytes = prepared.new.data.as_slice().unwrap_or_default();

    if !options.load_anyway
        && let Some(crossed) = crossed_line_limit(old_bytes, new_bytes, &options.limits)
    {
        // Under the byte ceiling by construction, so loading it anyway is always offered.
        return Ok(DiffContent::TooLarge {
            crossed,
            loadable: true,
        });
    }

    if let Some(content) = lfs_pointer(old_bytes, new_bytes) {
        return Ok(content);
    }

    Ok(text_diff(old_bytes, new_bytes, algorithm, options))
}

/// The exact answer, and the display-only overlay beside it.
fn text_diff(
    old_bytes: &[u8],
    new_bytes: &[u8],
    algorithm: Algorithm,
    options: &ContentOptions,
) -> DiffContent {
    let changes = exact_changes(old_bytes, new_bytes, algorithm);
    // `split_lines` splits on `\n` exactly as the tokens above were split, so there is one
    // line per token on each side and every range lands inside its own side.
    let text = TextDiff::new(split_lines(old_bytes), split_lines(new_bytes), changes);
    let highlights = super::intraline::highlights(&text, options.limits.max_line_bytes);
    let ignoring_whitespace = options
        .ignore_whitespace
        .then(|| super::whitespace::changes_ignoring_whitespace(old_bytes, new_bytes, algorithm));
    DiffContent::Text {
        text,
        overlay: DisplayOverlay::new(ignoring_whitespace, highlights),
    }
}

/// gix computes this and Cairn groups it (L3). The tokens keep their terminators (R2.4),
/// which is what makes a last line that lost its newline a change, exactly as git sees it.
fn exact_changes(old_bytes: &[u8], new_bytes: &[u8], algorithm: Algorithm) -> Vec<ChangedRange> {
    let mut input: InternedInput<&[u8]> = InternedInput::default();
    input.update_before(gix::diff::blob::sources::byte_lines(old_bytes));
    input.update_after(gix::diff::blob::sources::byte_lines(new_bytes));
    // `Diff::compute` then `postprocess_lines`, which is git's indent heuristic.
    let diff = gix::diff::blob::diff_with_slider_heuristics(algorithm, &input);
    changed_ranges(&diff)
}

/// gix's hunk list as the model's changed ranges. Every hunk advances both sides over the
/// same unchanged tokens, so the runs between two ranges are equal on both sides — which is
/// what `TextDiff::new` requires of whoever produces them.
pub(super) fn changed_ranges(diff: &gix::diff::blob::Diff) -> Vec<ChangedRange> {
    diff.hunks()
        .map(|hunk| {
            ChangedRange::new(
                LineSpan::at(hunk.before.start, hunk.before.end - hunk.before.start),
                LineSpan::at(hunk.after.start, hunk.after.end - hunk.after.start),
            )
        })
        .collect()
}

/// The size of one side's blob, read from its header so the object is never inflated.
fn blob_size(repo: &gix::Repository, id: &Oid, file: &ChangedFile) -> Result<u64, Error> {
    repo.find_header(object_id(id)?)
        .map(|header| header.size())
        .map_err(|source| Error::DiffFile {
            path: file.new_path.to_string(),
            source: Box::new(source),
        })
}

/// A side that does not exist is set with the null id, which gix reads as missing content —
/// the old side of an added file, the new side of a deleted one.
fn set_side(
    repo: &gix::Repository,
    cache: &mut gix::diff::blob::Platform,
    file: &ChangedFile,
    kind: ResourceKind,
) -> Result<(), Error> {
    let (id, mode, path) = match kind {
        ResourceKind::OldOrSource => (file.old_id, file.old_mode, &file.old_path),
        ResourceKind::NewOrDestination => (file.new_id, file.new_mode, &file.new_path),
    };
    let id = match id {
        Some(id) => object_id(&id)?,
        None => gix::hash::ObjectId::null(repo.object_hash()),
    };
    cache
        .set_resource(
            id,
            entry_kind(mode),
            path.as_bytes().into(),
            kind,
            &repo.objects,
        )
        .map_err(|source| Error::DiffFile {
            path: path.to_string(),
            source: Box::new(source),
        })
}

/// What gix is told the resource is. A missing side has no mode of its own, and a blob is
/// the harmless default: it decides how the bytes are read, and there are none.
fn entry_kind(mode: Option<FileMode>) -> EntryKind {
    match mode {
        Some(FileMode::Executable) => EntryKind::BlobExecutable,
        Some(FileMode::Symlink) => EntryKind::Link,
        // A submodule never reaches here: it is answered before anything is set.
        Some(FileMode::Regular) | Some(FileMode::Submodule) | None => EntryKind::Blob,
    }
}

fn byte_size(data: gix::diff::blob::platform::resource::Data<'_>) -> u64 {
    use gix::diff::blob::platform::resource::Data;
    match data {
        Data::Binary { size } => size,
        Data::Buffer { buf, .. } => buf.len() as u64,
        Data::Missing => 0,
    }
}

/// R2.6's two line rules, over git's form of the content. Measured without splitting the
/// file into lines, so refusing a file costs a scan and no allocation.
fn crossed_line_limit(old: &[u8], new: &[u8], limits: &DiffLimits) -> Option<SizeLimit> {
    let mut lines = 0u32;
    let mut longest = 0u32;
    for side in [old, new] {
        let mut side_lines = 0u32;
        for line in gix::diff::blob::sources::byte_lines(side) {
            side_lines = side_lines.saturating_add(1);
            // Without the terminator, which is how a `DiffLine` holds a line and what a
            // view would have to draw. A `\r` of a CRLF ending is part of the line.
            let bytes = line.strip_suffix(b"\n").unwrap_or(line);
            longest = longest.max(u32::try_from(bytes.len()).unwrap_or(u32::MAX));
        }
        lines = lines.max(side_lines);
    }
    if lines > limits.max_lines {
        return Some(SizeLimit::Lines {
            limit: limits.max_lines,
            measured: lines,
        });
    }
    if longest > limits.max_line_bytes {
        return Some(SizeLimit::LineLength {
            limit: limits.max_line_bytes,
            measured: longest,
        });
    }
    None
}

/// `Some` only when every side that exists is a pointer: a file that became a pointer, or
/// stopped being one, is a content change with one real side, and drawing it as a pointer
/// would hide that side's lines.
fn lfs_pointer(old: &[u8], new: &[u8]) -> Option<DiffContent> {
    let read = |side: &[u8]| -> Option<Option<String>> {
        if side.is_empty() {
            return Some(None);
        }
        if side.len() <= LFS_MAX_BYTES && side.starts_with(LFS_PREFIX) {
            return Some(Some(String::from_utf8_lossy(side).into_owned()));
        }
        None
    };
    let (old, new) = (read(old)?, read(new)?);
    if old.is_none() && new.is_none() {
        return None;
    }
    Some(DiffContent::LfsPointer { old, new })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_line_limits_measure_the_longer_side_and_the_longest_line() {
        let limits = DiffLimits {
            max_bytes: 1024,
            max_lines: 3,
            max_line_bytes: 8,
            load_anyway_bytes: 4096,
        };
        assert_eq!(crossed_line_limit(b"a\nb\n", b"a\n", &limits), None);
        assert_eq!(
            crossed_line_limit(b"a\n", b"a\nb\nc\nd\n", &limits),
            Some(SizeLimit::Lines {
                limit: 3,
                measured: 4
            }),
            "the limit is crossed when EITHER side crosses it"
        );
        assert_eq!(
            crossed_line_limit(b"123456789\n", b"a\n", &limits),
            Some(SizeLimit::LineLength {
                limit: 8,
                measured: 9
            })
        );
        assert_eq!(
            crossed_line_limit(b"12345678\n", b"a\n", &limits),
            None,
            "a line exactly at the limit is inside it, and its newline is not part of it"
        );
        assert_eq!(
            crossed_line_limit(b"1234567\r\n", b"a\n", &limits),
            None,
            "the `\\r` of a CRLF ending IS part of the line, which is 8 bytes here"
        );
    }

    /// Caught by: drawing a pointer notice for a file whose other side is real content,
    /// which would hide every line of it.
    #[test]
    fn only_a_file_that_is_a_pointer_on_every_side_it_has_reads_as_one() {
        let pointer = b"version https://git-lfs.github.com/spec/v1\noid sha256:abc\nsize 12\n";
        let prose = b"hello\n";

        let both = lfs_pointer(pointer, pointer);
        assert!(matches!(both, Some(DiffContent::LfsPointer { .. })));

        let added = lfs_pointer(b"", pointer);
        let Some(DiffContent::LfsPointer { old, new }) = added else {
            panic!("an added pointer is a pointer: {added:?}");
        };
        assert_eq!(old, None);
        assert!(new.is_some());

        assert_eq!(
            lfs_pointer(prose, pointer),
            None,
            "one real side is content"
        );
        assert_eq!(lfs_pointer(pointer, prose), None);
        assert_eq!(lfs_pointer(prose, prose), None);
        assert_eq!(lfs_pointer(b"", b""), None, "two empty sides are not one");

        let oversized = [pointer.as_slice(), &vec![b'x'; LFS_MAX_BYTES]].concat();
        assert_eq!(
            lfs_pointer(&oversized, &oversized),
            None,
            "the format caps a pointer at 1 KiB; past it, it is a file that starts like one"
        );
    }

    #[test]
    fn a_missing_side_is_read_as_a_plain_blob_and_every_mode_maps() {
        assert_eq!(entry_kind(None), EntryKind::Blob);
        assert_eq!(entry_kind(Some(FileMode::Regular)), EntryKind::Blob);
        assert_eq!(
            entry_kind(Some(FileMode::Executable)),
            EntryKind::BlobExecutable
        );
        assert_eq!(entry_kind(Some(FileMode::Symlink)), EntryKind::Link);
    }
}
