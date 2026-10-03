//! One file's content, both versions, and which of their lines git says changed.
//!
//! gix reads the two versions — through its resource cache in `Mode::ToGit`, the mode that
//! never runs a textconv program and the form `git diff` compares and `git apply --cached`
//! expects (R2.3), built with `skip_internal_diff_if_external_is_configured` off, so a
//! `diff.<driver>.command` in the user's config is read and never run — and decides, before
//! any line is read, what is not text: a submodule, a mode change alone, a file too large
//! (from the object's header), a binary one by git's rules, an LFS pointer.
//!
//! Which lines changed is git's answer (the content-parity decision of 2026-10-03, which
//! amends packet decision L3): `git diff-tree -p` run as a read (`crate::reads::patches`),
//! every line it prints checked against the lines gix read. So are the whitespace-ignoring
//! ranges (`-w`) and each hunk's function context. Where there is only one answer git could
//! give, git is not asked: a file added, deleted or changed in type is one change of every
//! line (git's patch for a type change is a deletion and an addition), a side with no lines
//! makes the other side's every line the change, and two sides with the same blob have
//! none. Intra-line highlights stay gix's: git has no equivalent.

use std::collections::BTreeMap;

use cairn_model::{
    ChangeSet, ChangeStatus, ChangedFile, ChangedRange, Context, DiffContent, DiffLimits, DiffLine,
    DisplayOverlay, FileDiff, FileMode, FunctionContext, LineNumber, LineSpan, Oid, RepoPath,
    SizeLimit, TextDiff, split_lines,
};
use gix::diff::blob::{ResourceKind, platform::prepare_diff::Operation};
use gix::objs::tree::EntryKind;

use crate::object_id::object_id;
use crate::ops::GitBinary;
use crate::reads::{
    Algorithm, Detection, FilePatch, PatchQuery, PatchText, Reading, Scope, Submodules, patches,
};
use crate::{Cancel, Error, Repository};

use super::ContentOptions;
use super::algorithm::{Algorithms, PathAlgorithm};
use super::hunk_grouping::Grouping;
use super::submodules::Hiding;

/// A Git LFS pointer names its own version first; the format caps a pointer at 1 KiB.
const LFS_PREFIX: &[u8] = b"version https://git-lfs.github.com/spec/";
const LFS_MAX_BYTES: usize = 1024;

/// The two commits a content query's file changed between, as `git` is given them.
#[derive(Debug, Clone, Copy)]
pub(super) struct Trees {
    pub(super) old: Oid,
    pub(super) new: Oid,
}

/// What reading a file's two versions decided before git is asked anything.
enum Prepared {
    /// Not text, or not to be drawn: the whole answer.
    Done(DiffContent),
    /// Both versions as lines, and whether git has to say which changed.
    Text {
        old: Vec<DiffLine>,
        new: Vec<DiffLine>,
        asks_git: bool,
    },
}

/// One file's content query (R2.3 through R2.8).
pub(super) fn file_diff(
    repo: &Repository,
    cache: &mut gix::diff::blob::Platform,
    git: &GitBinary,
    trees: Trees,
    file: &ChangedFile,
    options: &ContentOptions,
    cancel: &impl Cancel,
) -> Result<FileDiff, Error> {
    if cancel.is_cancelled() {
        return Err(Error::ContentCancelled);
    }
    // Read first, as `git diff` reads its configuration before it diffs anything: a value
    // it refuses refuses every file.
    let grouping = Grouping::read(repo.inner())?;
    let content = match prepare(repo, cache, file, options)? {
        Prepared::Done(content) => content,
        Prepared::Text { old, new, asks_git } => {
            let readings = if asks_git {
                let algorithm = Algorithms::read(repo.inner(), git.version())?
                    .for_old_paths(git, repo, &[&file.old_path], cancel)?
                    .pop()
                    .and_then(PathAlgorithm::flag);
                let asker = Asker {
                    git,
                    repo,
                    trees,
                    options,
                    cancel,
                };
                Some(asker.file(file, algorithm, &old, &new)?)
            } else {
                None
            };
            text_content(old, new, readings, options, grouping.inter_hunk_context)
        }
    };
    Ok(FileDiff {
        file: file.clone(),
        content,
    })
}

/// Every file of a change set, for Expand All: decided file by file as [`file_diff`]
/// decides one, with the lines of every text file git has to read asked of git in as few
/// `diff-tree -p` runs as the algorithms allow — ONE over the whole comparison for every
/// file diffed with `diff.algorithm`, with the changes query's own detection, so git pairs
/// and lists exactly what that query listed; and one per distinct algorithm the files'
/// diff drivers name, over those files' paths, since one run cannot pass two algorithms
/// (and a run that leaves a driver to apply its own carries it into every later file,
/// `docs/systems/diff.md`). A file a run's answer does not hold as the change set holds it
/// — paired otherwise, which a hidden submodule's place in a cut-short rename search or a
/// group's narrower paths can do, or called binary where gix called it text — is asked
/// about on its own. `cancel` is checked between files while they are read, before each
/// file asked about on its own, and between files as the answers are assembled, and
/// polled while git runs.
pub(super) fn file_diffs(
    repo: &Repository,
    cache: &mut gix::diff::blob::Platform,
    git: &GitBinary,
    trees: Trees,
    set: &ChangeSet,
    options: &ContentOptions,
    cancel: &impl Cancel,
) -> Result<Vec<FileDiff>, Error> {
    let grouping = Grouping::read(repo.inner())?;
    let mut prepared = Vec::with_capacity(set.files.len());
    for file in &set.files {
        if cancel.is_cancelled() {
            return Err(Error::ContentCancelled);
        }
        prepared.push(prepare(repo, cache, file, options)?);
    }
    let asking: Vec<usize> = prepared
        .iter()
        .enumerate()
        .filter(|(_, prepared)| matches!(prepared, Prepared::Text { asks_git: true, .. }))
        .map(|(index, _)| index)
        .collect();

    let algorithms = Algorithms::read(repo.inner(), git.version())?;
    let old_paths: Vec<&RepoPath> = asking
        .iter()
        .filter_map(|index| set.files.get(*index))
        .map(|file| &file.old_path)
        .collect();
    let chosen = algorithms.for_old_paths(git, repo, &old_paths, cancel)?;
    let mut together: Vec<usize> = Vec::new();
    // Each driver algorithm in the order it was first met, with its files.
    let mut by_driver: Vec<(Algorithm, Vec<usize>)> = Vec::new();
    for (index, path_algorithm) in asking.iter().zip(chosen) {
        match path_algorithm {
            PathAlgorithm::Configured(_) => together.push(*index),
            PathAlgorithm::Driver(algorithm) => {
                match by_driver.iter_mut().find(|(held, _)| *held == algorithm) {
                    Some((_, group)) => group.push(*index),
                    None => by_driver.push((algorithm, vec![*index])),
                }
            }
        }
    }

    let asker = Asker {
        git,
        repo,
        trees,
        options,
        cancel,
    };
    let held = Held {
        set,
        prepared: &prepared,
    };
    let mut readings: BTreeMap<usize, (Reading, Option<Reading>)> = BTreeMap::new();
    let mut alone: Vec<(usize, Option<Algorithm>)> = Vec::new();
    if !together.is_empty() {
        let hiding = Hiding::read(repo.inner(), repo.workdir().is_some())?;
        let comparison = Scope::Comparison {
            detection: detection_of(set),
            submodules: match hiding {
                Hiding::EveryGitlink => Submodules::HideEvery,
                Hiding::Nothing | Hiding::GitlinksExcept(_) => Submodules::AsListed,
            },
        };
        let configured = Some(algorithms.configured());
        asker.many(
            &held,
            &together,
            (comparison, configured),
            configured,
            &mut readings,
            &mut alone,
        )?;
    }
    for (algorithm, group) in &by_driver {
        let files: Vec<&ChangedFile> = group
            .iter()
            .filter_map(|index| set.files.get(*index))
            .collect();
        let scope = Scope::Paths {
            files: &files,
            detection: detection_of(set),
        };
        // Alone, a driver's file passes no flag and git applies the driver itself, as the
        // user's `git diff -- <path>` does; the group's run names the same algorithm.
        asker.many(
            &held,
            group,
            (scope, Some(*algorithm)),
            None,
            &mut readings,
            &mut alone,
        )?;
    }
    for (index, algorithm) in alone {
        if cancel.is_cancelled() {
            return Err(Error::ContentCancelled);
        }
        let (Some(file), Some(Prepared::Text { old, new, .. })) =
            (set.files.get(index), prepared.get(index))
        else {
            continue;
        };
        readings.insert(index, asker.file(file, algorithm, old, new)?);
    }

    // Each file's lines, ranges and intra-line highlights are built here, after git has
    // answered: work proportional to the whole commit, so it is cancellable file by file
    // like the reads before it.
    let mut diffs = Vec::with_capacity(set.files.len());
    for (index, (file, prepared)) in set.files.iter().zip(prepared).enumerate() {
        if cancel.is_cancelled() {
            return Err(Error::ContentCancelled);
        }
        diffs.push(FileDiff {
            file: file.clone(),
            content: match prepared {
                Prepared::Done(content) => content,
                Prepared::Text { old, new, .. } => text_content(
                    old,
                    new,
                    readings.remove(&index),
                    options,
                    grouping.inter_hunk_context,
                ),
            },
        });
    }
    Ok(diffs)
}

/// The change set and what reading each of its files decided, which a run over several
/// files is read against.
struct Held<'a> {
    set: &'a ChangeSet,
    prepared: &'a [Prepared],
}

/// The changes query's detection, as its answer reports it: what `diff.renames` asked for,
/// under the `-l` git was given (`0` where nothing limited the search).
fn detection_of(set: &ChangeSet) -> Detection {
    let limit = set.renames.limit.unwrap_or(0);
    match (set.renames.enabled, set.renames.copies) {
        (false, _) => Detection::Off,
        (true, false) => Detection::Renames { limit },
        (true, true) => Detection::Copies { limit },
    }
}

fn by_paths(patches: Vec<FilePatch>) -> BTreeMap<(RepoPath, RepoPath), FilePatch> {
    patches
        .into_iter()
        .map(|patch| (key(&patch.file), patch))
        .collect()
}

fn key(file: &ChangedFile) -> (RepoPath, RepoPath) {
    (file.old_path.clone(), file.new_path.clone())
}

/// Whether git's record is the file the caller holds: the same kind of change between the
/// same blobs. A similarity score may differ between a search over two paths and one over
/// a whole comparison; nothing else may.
fn same_file(found: &ChangedFile, held: &ChangedFile) -> bool {
    std::mem::discriminant(&found.status) == std::mem::discriminant(&held.status)
        && found.old_path == held.old_path
        && found.new_path == held.new_path
        && found.old_id == held.old_id
        && found.new_id == held.new_id
}

/// The context `git` is asked at: the view's, never less than one, and one for the entire
/// file, whose one hunk starts at the first line, where git prints no function context.
pub(super) fn git_context(context: Context) -> u32 {
    context.line_count().unwrap_or(1).max(1)
}

/// What every patch read of one query shares: the git to run, where, between which
/// commits, at which context, and the query's cancel.
struct Asker<'a, C: Cancel> {
    git: &'a GitBinary,
    repo: &'a Repository,
    trees: Trees,
    options: &'a ContentOptions,
    cancel: &'a C,
}

impl<C: Cancel> Asker<'_, C> {
    fn query<'q>(
        &'q self,
        algorithm: Option<Algorithm>,
        ignore_whitespace: bool,
        scope: Scope<'q>,
    ) -> PatchQuery<'q> {
        PatchQuery {
            old: &self.trees.old,
            new: &self.trees.new,
            context: git_context(self.options.context),
            algorithm,
            ignore_whitespace,
            scope,
        }
    }

    /// One run over several files (`indices` of the change set), `-w` beside it when the
    /// caller asked for the whitespace-ignoring reading: each file the answer holds as the
    /// change set does gets its readings; any other is left in `alone`, to be asked about
    /// on its own with `alone_algorithm`.
    fn many(
        &self,
        held: &Held<'_>,
        indices: &[usize],
        (scope, algorithm): (Scope<'_>, Option<Algorithm>),
        alone_algorithm: Option<Algorithm>,
        readings: &mut BTreeMap<usize, (Reading, Option<Reading>)>,
        alone: &mut Vec<(usize, Option<Algorithm>)>,
    ) -> Result<(), Error> {
        let ask = |ignore_whitespace: bool| {
            let query = self.query(algorithm, ignore_whitespace, scope);
            patches(self.git, self.repo, &query, self.cancel).map(by_paths)
        };
        let exact = ask(false)?;
        let ignoring = if self.options.ignore_whitespace {
            Some(ask(true)?)
        } else {
            None
        };
        for &index in indices {
            let (Some(file), Some(Prepared::Text { old, new, .. })) =
                (held.set.files.get(index), held.prepared.get(index))
            else {
                continue;
            };
            let Found::Text(exact_text) = lookup(&exact, file) else {
                alone.push((index, alone_algorithm));
                continue;
            };
            let ignoring_text = match ignoring.as_ref().map(|ignoring| lookup(ignoring, file)) {
                None => None,
                Some(Found::Text(text)) => Some(Some(text)),
                // Every change was whitespace, which git says by leaving the file out.
                Some(Found::Left | Found::Unprinted) => Some(None),
                Some(Found::Unusable) => {
                    alone.push((index, alone_algorithm));
                    continue;
                }
            };
            let exact = against(file, exact_text, old, new, false)?;
            let ignoring = match ignoring_text {
                None => None,
                Some(None) => Some(Reading::default()),
                Some(Some(text)) => Some(against(file, text, old, new, true)?),
            };
            readings.insert(index, (exact, ignoring));
        }
        Ok(())
    }

    /// One file asked of git on its own: the exact reading, and the whitespace-ignoring
    /// one when the caller asked for it.
    fn file(
        &self,
        file: &ChangedFile,
        algorithm: Option<Algorithm>,
        old: &[DiffLine],
        new: &[DiffLine],
    ) -> Result<(Reading, Option<Reading>), Error> {
        let ask = |ignore_whitespace: bool| -> Result<Reading, Error> {
            let query = self.query(algorithm, ignore_whitespace, Scope::File(file));
            let answer = by_paths(patches(self.git, self.repo, &query, self.cancel)?);
            match (lookup(&answer, file), ignore_whitespace) {
                (Found::Text(text), _) => against(file, text, old, new, ignore_whitespace),
                (Found::Left | Found::Unprinted, true) => Ok(Reading::default()),
                (Found::Left, false) => Err(Error::ContentReadsDisagree {
                    path: file.new_path.to_string(),
                    detail: "git did not list the change the changes query listed".to_owned(),
                }),
                (Found::Unprinted | Found::Unusable, _) => Err(Error::ContentReadsDisagree {
                    path: file.new_path.to_string(),
                    detail: "git printed no diff of the two contents".to_owned(),
                }),
            }
        };
        let exact = ask(false)?;
        let ignoring = if self.options.ignore_whitespace {
            Some(ask(true)?)
        } else {
            None
        };
        Ok((exact, ignoring))
    }
}

/// What git's answer holds for one file.
enum Found<'p> {
    /// The file's own patch, a diff of its two contents.
    Text(&'p PatchText),
    /// Not listed at all. Under `-w`, every change was whitespace (a git that leaves such a
    /// file out of its raw records); otherwise, git paired the paths another way.
    Left,
    /// Listed with no patch: under `-w`, every change was whitespace (a git that lists such
    /// a file and prints nothing for it).
    Unprinted,
    /// Listed as another change, or printed as binary.
    Unusable,
}

fn lookup<'p>(
    patches: &'p BTreeMap<(RepoPath, RepoPath), FilePatch>,
    file: &ChangedFile,
) -> Found<'p> {
    let Some(patch) = patches.get(&key(file)) else {
        return Found::Left;
    };
    if !same_file(&patch.file, file) {
        return Found::Unusable;
    }
    match &patch.text {
        None => Found::Unprinted,
        Some(text) if text.is_binary() => Found::Unusable,
        Some(text) => Found::Text(text),
    }
}

fn against(
    file: &ChangedFile,
    text: &PatchText,
    old: &[DiffLine],
    new: &[DiffLine],
    whitespace_ignored: bool,
) -> Result<Reading, Error> {
    text.read_against(old, new, whitespace_ignored)
        .map_err(|detail| Error::ContentReadsDisagree {
            path: file.new_path.to_string(),
            detail,
        })
}

/// The exact answer, and the display-only overlay beside it. `readings` is git's, exact
/// and whitespace-ignoring; `None` is a file git was not asked about, whose one possible
/// answer is every line of one side against every line of the other. The function context
/// carries the user's `diff.interHunkContext`, which a view groups the hunks with.
pub(super) fn text_content(
    old: Vec<DiffLine>,
    new: Vec<DiffLine>,
    readings: Option<(Reading, Option<Reading>)>,
    options: &ContentOptions,
    inter_hunk_context: u32,
) -> DiffContent {
    let (exact, ignoring) = match readings {
        Some(readings) => readings,
        None => {
            let whole = whole(&old, &new);
            let ignoring = options.ignore_whitespace.then(|| whole.clone());
            (whole, ignoring)
        }
    };
    let mut starts = exact.function_context;
    let ignoring_changes = ignoring.map(|reading| {
        starts.extend(reading.function_context);
        reading.changes
    });
    let text = TextDiff::new(old, new, exact.changes);
    let highlights = super::intraline::highlights(&text, options.limits.max_line_bytes);
    let function_context =
        FunctionContext::read_at(Context::Lines(git_context(options.context)), starts)
            .with_inter_hunk_context(inter_hunk_context);
    DiffContent::Text {
        text,
        overlay: DisplayOverlay::new(ignoring_changes, highlights)
            .with_function_context(function_context),
    }
}

/// What git says of a file it is not asked about: nothing changed when the two sides are
/// the same lines — two empty files, or a rename that kept its blob — otherwise every line
/// of one side against every line of the other, in one hunk starting at the first line,
/// where git prints no function context.
fn whole(old: &[DiffLine], new: &[DiffLine]) -> Reading {
    if old == new {
        return Reading::default();
    }
    let lines = |side: &[DiffLine]| u32::try_from(side.len()).unwrap_or(u32::MAX);
    Reading {
        changes: vec![ChangedRange::new(
            LineSpan::at(0, lines(old)),
            LineSpan::at(0, lines(new)),
        )],
        function_context: vec![(LineNumber::from_index(0), Vec::new())],
    }
}

fn prepare(
    repo: &Repository,
    cache: &mut gix::diff::blob::Platform,
    file: &ChangedFile,
    options: &ContentOptions,
) -> Result<Prepared, Error> {
    // A submodule is a commit id in a tree, not a blob; gix's blob platform refuses the
    // mode outright, so this is decided before anything is read.
    if file.old_mode == Some(FileMode::Submodule) || file.new_mode == Some(FileMode::Submodule) {
        return Ok(Prepared::Done(DiffContent::Submodule {
            old_target: file.old_id,
            new_target: file.new_id,
            dirty: false,
        }));
    }

    // Same blob on both sides and a different mode: the whole change is the mode, and
    // reading the content would only prove it is identical. On a commit that renames
    // 27,592 files this is the difference between a file list and inflating every blob.
    if let (Some(old), Some(new)) = (file.old_id, file.new_id)
        && old == new
        && file.mode_changed()
    {
        return Ok(Prepared::Done(DiffContent::ModeChangeOnly));
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
            return Ok(Prepared::Done(DiffContent::TooLarge {
                crossed: SizeLimit::Bytes {
                    limit: ceiling,
                    measured: size,
                },
                loadable: !options.load_anyway && size <= options.limits.load_anyway_bytes,
            }));
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
    match prepared.operation {
        // git's own rule: the `diff`/`binary` attribute, `core.bigFileThreshold`, or a NUL
        // byte in the first 8,000 bytes (R2.5). gix clears the buffer of a binary resource,
        // so the size is all there is to show.
        Operation::SourceOrDestinationIsBinary => {
            return Ok(Prepared::Done(DiffContent::Binary {
                old_size: byte_size(prepared.old.data),
                new_size: byte_size(prepared.new.data),
            }));
        }
        // Unreachable while the cache is built with `skip_internal_diff_if_external_is_configured`
        // off, which is what `gix::diff::resource_cache` does and what R2.3 requires: the
        // configured program is read and never started. Answered rather than asserted, so a
        // gix that changed that default draws a notice instead of running a program.
        Operation::ExternalCommand { command } => {
            return Ok(Prepared::Done(DiffContent::Unsupported {
                reason: format!(
                    "an external diff program is configured for this path ({command}), and Cairn \
                     never runs one"
                ),
            }));
        }
        // The algorithm gix would use is not the one asked of git: `crate::diff::algorithm`
        // decides that the way the user's `git diff` does.
        Operation::InternalDiff { .. } => {}
    }

    let old_bytes = prepared.old.data.as_slice().unwrap_or_default();
    let new_bytes = prepared.new.data.as_slice().unwrap_or_default();

    if !options.load_anyway
        && let Some(crossed) = crossed_line_limit(old_bytes, new_bytes, &options.limits)
    {
        // Under the byte ceiling by construction, so loading it anyway is always offered.
        return Ok(Prepared::Done(DiffContent::TooLarge {
            crossed,
            loadable: true,
        }));
    }

    if let Some(content) = lfs_pointer(old_bytes, new_bytes) {
        return Ok(Prepared::Done(content));
    }

    let (old, new) = (split_lines(old_bytes), split_lines(new_bytes));
    let one_answer = matches!(
        file.status,
        ChangeStatus::Added | ChangeStatus::Deleted | ChangeStatus::TypeChanged
    ) || file.old_id == file.new_id
        || old.is_empty()
        || new.is_empty();
    Ok(Prepared::Text {
        old,
        new,
        asks_git: !one_answer,
    })
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
pub(super) fn crossed_line_limit(old: &[u8], new: &[u8], limits: &DiffLimits) -> Option<SizeLimit> {
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
pub(super) fn lfs_pointer(old: &[u8], new: &[u8]) -> Option<DiffContent> {
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

    /// The stale-read guard, as the caller meets it: a line git printed that is not the
    /// line gix read is the typed error a caller retries, naming the file, and the same
    /// patch over the lines git did print is read. Caught by: a mismatch mapped to some
    /// other error, or swallowed into an empty diff.
    #[test]
    fn lines_git_printed_that_were_not_read_are_the_error_a_caller_retries() {
        let mut output = format!(
            ":100644 100644 {} {} M\0f.txt\0\0",
            "07da224c7ec04501dfb451be161fa962effe1dc1", "ebc3711f1349e6204d89c344c178221fb8bf5113"
        )
        .into_bytes();
        output.extend_from_slice(b"diff --git a/f.txt b/f.txt\n@@ -1,2 +1,2 @@\n a\n-b\n+B\n");
        let parsed = crate::reads::parse_patches(&output).unwrap();
        let (file, text) = (&parsed[0].file, parsed[0].text.as_ref().unwrap());
        let new = split_lines(b"a\nB\n");

        let read = against(file, text, &split_lines(b"a\nb\n"), &new, false).unwrap();
        assert_eq!(
            read.changes,
            vec![ChangedRange::new(LineSpan::at(1, 1), LineSpan::at(1, 1))]
        );
        let changed_since = against(file, text, &split_lines(b"a\nb!\n"), &new, false);
        match changed_since {
            Err(Error::ContentReadsDisagree { path, .. }) => assert_eq!(path, "f.txt"),
            other => panic!("expected the content-changed error, got {other:?}"),
        }
    }

    /// A file git is not asked about has the one answer git would give.
    #[test]
    fn a_file_with_one_possible_answer_is_every_line_against_every_line() {
        assert_eq!(whole(&[], &[]), Reading::default());
        let added = whole(&[], &split_lines(b"x\ny\n"));
        assert_eq!(
            added.changes,
            vec![ChangedRange::new(LineSpan::at(0, 0), LineSpan::at(0, 2))]
        );
        assert_eq!(
            added.function_context,
            vec![(LineNumber::from_index(0), Vec::new())],
            "a hunk starting at the first line has no function context in git"
        );
        let emptied = whole(&split_lines(b"x\n"), &[]);
        assert_eq!(
            emptied.changes,
            vec![ChangedRange::new(LineSpan::at(0, 1), LineSpan::at(0, 0))]
        );
    }

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
            crossed_line_limit(b"a\nb\nc\n", b"a\n", &limits),
            None,
            "a side of exactly the limit's lines is inside it"
        );
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
