//! One path's working-tree diffs (R3): what is staged, what is not, and an untracked file.
//!
//! git answers all three (`crate::reads::working_tree`), and the side git reads from the
//! working tree is never read by Cairn: its lines are rebuilt from git's own patch over the
//! old side, so they are the content in git's form — after the clean filter driver the
//! path's attributes name, line-ending conversion, `ident` and a working-tree encoding —
//! exactly what the user's `git diff` compares (L6, D1 as amended in
//! `docs/design/engine.md`). gix reads the index, fresh for every query (R3.3), to say what
//! git's answer does not say alone, or what plumbing says otherwise than porcelain: a
//! conflicted path (answered as conflicted, never diffed against one stage), a sparse index
//! (unsupported, and said so), an intent-to-add entry (which `git diff --cached` does not
//! list), and the sizes of the blobs on each side, so a file too large to draw is refused
//! before git diffs it. gix also reads every blob git names, by the id git names it with.
//!
//! The staged side pairs a rename or a copy as the user's `git diff --cached` pairs it, under
//! their `diff.renames` and `diff.renameLimit` (R2.6, L18): which pair the path belongs to is
//! asked of the whole index first (`crate::reads::staged_pairing`), and that pair's diff is
//! then read across both its paths, so a staged rename is drawn as git draws it and not as an
//! addition beside a deletion (`crate::reads`' working-tree module, "Renames").
//!
//! The stale-read guard is three checks. Every line git prints is checked against the side
//! it belongs to (`PatchText::new_side`, `PatchText::read_against`). The rebuilt working-tree
//! side is hashed and must be the object id git printed for it on the patch's `index` line,
//! which git computes from a second read of the file — so a file changed between git's two
//! reads of it, or a rebuild that is not the content git diffed, is
//! [`Error::ContentReadsDisagree`]. And the whitespace-ignoring read, a second process, must
//! name the same object for that side. Nothing here writes: no gix call in this module
//! writes, and git's reads are plumbing that refreshes no index and writes no object.

use cairn_model::{
    ChangeStatus, ChangedFile, DiffContent, DiffLine, FileDiff, FileMode, Oid, RepoPath, SizeLimit,
    split_lines,
};
use gix::bstr::ByteSlice as _;

use crate::object_id::{model_id, object_id};
use crate::ops::GitBinary;
use crate::reads::{
    Detection, Paired, PatchText, Reading, Side, WorkingTreeAnswer, WorkingTreeQuery,
    staged_pairing, work_tree_relative, working_tree_patch,
};
use crate::{Cancel, Error, Repository};

use super::algorithm::{Algorithms, PathAlgorithm};
use super::content::{crossed_line_limit, git_context, lfs_pointer, text_content};
use super::hunk_grouping::Grouping;
use super::renames::Configured;
use super::submodules::working_tree_ignore;
use super::{ContentOptions, WorkingTreeDiff};

/// Room in git's output for what is not a line of content: the raw record and the patch's
/// header lines, each naming the path a few times.
const HEADER_ROOM: u64 = 64 * 1024;

/// One path's working-tree diff, or `None` where git's own diff of it is empty.
pub(super) fn working_tree_diff(
    repo: &Repository,
    git: &GitBinary,
    path: &RepoPath,
    which: WorkingTreeDiff,
    options: &ContentOptions,
    cancel: &impl Cancel,
) -> Result<Option<FileDiff>, Error> {
    if cancel.is_cancelled() {
        return Err(Error::ContentCancelled);
    }
    // Read first, as `git diff` reads its configuration before it diffs anything.
    let grouping = Grouping::read(repo.inner())?;
    if repo.workdir().is_none() {
        return Ok(Some(stand_in(
            path,
            DiffContent::Unsupported {
                reason: "a bare repository has no working tree".to_owned(),
            },
        )));
    }
    if which == WorkingTreeDiff::Untracked {
        work_tree_relative(path)?;
        if let Some(reason) = not_a_file(repo, path) {
            return Ok(Some(stand_in(path, DiffContent::Unsupported { reason })));
        }
    }
    let inner = repo.inner();

    // What the index says first: it decides some answers outright, and names the blob a
    // staged or an unstaged diff has on one side.
    let mut staged_commit: Option<Oid> = None;
    let mut pair: Option<(Detection, ChangedFile)> = None;
    let sizes = match which {
        WorkingTreeDiff::Untracked => Sizes {
            old: Some(0),
            new: None,
        },
        WorkingTreeDiff::Staged | WorkingTreeDiff::Unstaged => {
            let index = fresh_index(inner)?;
            if index.is_sparse() {
                return Ok(Some(stand_in(
                    path,
                    DiffContent::Unsupported {
                        reason: "the index is a sparse index, which Cairn does not read yet"
                            .to_owned(),
                    },
                )));
            }
            let entries = index
                .entry_range(path.as_bytes().as_bstr())
                .and_then(|range| index.entries().get(range))
                .unwrap_or_default();
            if entries
                .iter()
                .any(|entry| entry.stage() != gix::index::entry::Stage::Unconflicted)
            {
                return Ok(Some(stand_in(path, DiffContent::Conflicted)));
            }
            let entry = entries.first();
            let intent_to_add = entry.is_some_and(|entry| {
                entry
                    .flags
                    .contains(gix::index::entry::Flags::INTENT_TO_ADD)
            });
            let index_size = match entry {
                Some(entry) if !entry.mode.is_submodule() && !intent_to_add => {
                    size_of(inner, entry.id, path)?
                }
                Some(_) | None => 0,
            };
            if which == WorkingTreeDiff::Staged {
                // `git diff --cached` lists no intent-to-add entry, where `diff-index
                // --cached` lists it as an empty file added (`crate::reads::working_tree`).
                if intent_to_add {
                    return Ok(None);
                }
                let (commit, head_blob) = head_side(inner, path)?;
                staged_commit = Some(commit);
                // R2.6: the pair `git diff --cached` puts the path in, if any; its sides are
                // the pair's two blobs, at two paths.
                let detection = Configured::read(inner)?.search(git.version()).detection();
                if let Some(paired) = staged_pairing(git, repo, &commit, detection, path, cancel)? {
                    let sizes = Sizes {
                        old: Some(side_size(inner, &paired.old_id, path)?),
                        new: Some(side_size(inner, &paired.new_id, path)?),
                    };
                    pair = Some((detection, paired));
                    sizes
                } else {
                    Sizes {
                        old: Some(match head_blob {
                            Some(id) => size_of(inner, id, path)?,
                            None => 0,
                        }),
                        new: Some(index_size),
                    }
                }
            } else {
                Sizes {
                    old: Some(index_size),
                    new: None,
                }
            }
        }
    };
    let side = match (which, &staged_commit) {
        (WorkingTreeDiff::Staged, Some(commit)) => Side::Staged { commit },
        (WorkingTreeDiff::Untracked, _) => Side::Untracked,
        (WorkingTreeDiff::Staged | WorkingTreeDiff::Unstaged, _) => Side::Unstaged,
    };
    // A pair's diff driver is its source's, as git reads it for the old side.
    let old_path = pair.as_ref().map_or(path, |(_, paired)| &paired.old_path);
    let (algorithm, ignore_submodules) = if side == Side::Untracked {
        // Every line of an untracked file is added whatever the algorithm, and it has no
        // submodule to ignore.
        (None, None)
    } else {
        (
            Algorithms::read(inner, git.version())?
                .for_old_paths(git, repo, &[old_path], cancel)?
                .pop()
                .and_then(PathAlgorithm::flag),
            working_tree_ignore(inner, path)?,
        )
    };
    let asker = Asker {
        repo,
        git,
        path,
        pair: pair.as_ref().map(|(detection, paired)| {
            (
                Paired {
                    detection: *detection,
                    old: &paired.old_path,
                    new: &paired.new_path,
                },
                paired,
            )
        }),
        options,
        cancel,
        side,
        algorithm,
        ignore_submodules,
        inter_hunk_context: grouping.inter_hunk_context,
        ceiling: if options.load_anyway {
            options.limits.load_anyway_bytes
        } else {
            options.limits.max_bytes
        },
    };
    asker.answer(sizes)
}

/// The sizes of the two sides where they are known before git runs: a blob's from its
/// header, an absent side's as zero; `None` for a working-tree side, which only git reads.
#[derive(Debug, Clone, Copy)]
struct Sizes {
    old: Option<u64>,
    new: Option<u64>,
}

impl Sizes {
    fn largest_known(self) -> u64 {
        self.old.unwrap_or(0).max(self.new.unwrap_or(0))
    }
}

/// What one query asks git, and with what.
struct Asker<'a, C: Cancel> {
    repo: &'a Repository,
    git: &'a GitBinary,
    path: &'a RepoPath,
    /// The staged side's rename or copy, and the record the whole-index read found for it,
    /// which the paired read must answer again.
    pair: Option<(Paired<'a>, &'a ChangedFile)>,
    options: &'a ContentOptions,
    cancel: &'a C,
    side: Side<'a>,
    algorithm: Option<crate::reads::Algorithm>,
    ignore_submodules: Option<&'static str>,
    /// The user's `diff.interHunkContext`, read once for the query.
    inter_hunk_context: u32,
    /// R2.6's byte ceiling, or the load-anyway one.
    ceiling: u64,
}

impl<C: Cancel> Asker<'_, C> {
    fn ask(
        &self,
        ignore_whitespace: bool,
        raw_only: bool,
        output: u64,
    ) -> Result<WorkingTreeAnswer, Error> {
        let query = WorkingTreeQuery {
            side: self.side,
            path: self.path,
            paired: self.pair.map(|(paired, _)| paired),
            context: git_context(self.options.context),
            algorithm: self.algorithm,
            ignore_whitespace,
            ignore_submodules: self.ignore_submodules,
            raw_only,
            ceiling: usize::try_from(output).unwrap_or(usize::MAX),
        };
        let answer = working_tree_patch(self.git, self.repo, &query, self.cancel)?;
        self.same_pair(answer)
    }

    /// The paired read's record is the pair the whole-index read found, or the index moved
    /// between the two reads and the answer is asked again (`ContentReadsDisagree`).
    fn same_pair(&self, answer: WorkingTreeAnswer) -> Result<WorkingTreeAnswer, Error> {
        let Some((_, expected)) = self.pair else {
            return Ok(answer);
        };
        let found = match &answer {
            WorkingTreeAnswer::Unlisted => None,
            WorkingTreeAnswer::Listed { file, .. } => Some(file),
            WorkingTreeAnswer::PastCeiling { file } => file.as_ref(),
        };
        match found {
            Some(file) if file == expected => Ok(answer),
            // Git ended before its record arrived whole: nothing to compare.
            None if matches!(answer, WorkingTreeAnswer::PastCeiling { .. }) => Ok(answer),
            Some(_) | None => Err(self.disagree(
                "the index changed between git's two reads of a staged rename or copy".to_owned(),
            )),
        }
    }

    fn working_tree(&self) -> bool {
        !matches!(self.side, Side::Staged { .. })
    }

    fn answer(&self, sizes: Sizes) -> Result<Option<FileDiff>, Error> {
        // What git prints is at most every line of both sides with a marker before each,
        // so twice the two sides' bytes; a working-tree side counts at the ceiling, and
        // output past that much means a side is past it.
        let within = |side: Option<u64>| side.unwrap_or(self.ceiling).min(self.ceiling);
        let output = within(sizes.old)
            .saturating_add(within(sizes.new))
            .saturating_mul(2)
            .saturating_add(HEADER_ROOM);
        // R2.6: a blob too large to draw is refused before git diffs it, git asked only
        // whether the path changed and how. Only a working-tree modification needs the
        // patch after all, since its record cannot tell an edit from a stat or a mode
        // that alone moved — and `git diff` shows those as nothing and a mode change. A
        // staged record names both blobs, so one blob under two modes is the mode alone,
        // which `git diff --cached` shows as its mode lines and a commit answers the same.
        if sizes.largest_known() > self.ceiling {
            match self.ask(false, true, HEADER_ROOM)? {
                WorkingTreeAnswer::Unlisted => return Ok(None),
                WorkingTreeAnswer::Listed { file, .. }
                    if self.working_tree() && file.status == ChangeStatus::Modified => {}
                WorkingTreeAnswer::Listed { file, .. }
                    if file.status == ChangeStatus::Modified
                        && file.mode_changed()
                        && not_null(file.old_id).is_some()
                        && file.old_id == file.new_id =>
                {
                    return Ok(Some(FileDiff {
                        file,
                        content: DiffContent::ModeChangeOnly,
                    }));
                }
                WorkingTreeAnswer::Listed { file, .. } => {
                    return Ok(Some(FileDiff {
                        file: self.named(file, None),
                        content: self.too_large(sizes.largest_known()),
                    }));
                }
                WorkingTreeAnswer::PastCeiling { file } => {
                    return Ok(Some(self.past_ceiling(file, sizes)));
                }
            }
        }
        match self.ask(false, false, output)? {
            WorkingTreeAnswer::Unlisted => Ok(None),
            WorkingTreeAnswer::PastCeiling { file } => Ok(Some(self.past_ceiling(file, sizes))),
            WorkingTreeAnswer::Listed { file, sections } => {
                self.decide(file, &sections, sizes, output)
            }
        }
    }

    /// git's record and patch, decided into what the view draws.
    fn decide(
        &self,
        file: ChangedFile,
        sections: &[PatchText],
        sizes: Sizes,
        output: u64,
    ) -> Result<Option<FileDiff>, Error> {
        // git printed no patch: a stat-only change `diff-files` lists, which the user's
        // `git diff` shows as nothing.
        let Some(first) = sections.first() else {
            return Ok(None);
        };
        let inner = self.repo.inner();

        if file.old_mode == Some(FileMode::Submodule) || file.new_mode == Some(FileMode::Submodule)
        {
            return Ok(Some(self.submodule(file, sections)));
        }

        if sections.iter().any(PatchText::is_binary) {
            let old_size = self.old_size(&file)?;
            let new_size = match (self.working_tree(), file.new_id) {
                _ if file.new_mode.is_none() => 0,
                (true, _) => self.size_on_disk(),
                (false, Some(id)) => size_of(inner, object_id(&id)?, self.path)?,
                (false, None) => 0,
            };
            let named = sections.last().and_then(PatchText::new_index_id);
            let file = self.named(file, named);
            let largest = old_size.max(new_size);
            let content = if largest > self.ceiling {
                self.too_large(largest)
            } else {
                DiffContent::Binary { old_size, new_size }
            };
            return Ok(Some(FileDiff { file, content }));
        }

        if file.status != ChangeStatus::TypeChanged
            && file.mode_changed()
            && !first.has_hunks()
            && first.new_index_id().is_none()
        {
            let file = self.named(file.clone(), file.old_id);
            return Ok(Some(FileDiff {
                file,
                content: DiffContent::ModeChangeOnly,
            }));
        }

        // A side already known to be past the ceiling is not read.
        if sizes.largest_known() > self.ceiling {
            return Ok(Some(FileDiff {
                file: self.named(file, None),
                content: self.too_large(sizes.largest_known()),
            }));
        }

        // Both sides as lines in git's form: a blob as stored, a working-tree side rebuilt
        // from git's patch and checked against the object id git named for it.
        let old_bytes = match file.old_id.filter(|_| file.old_mode.is_some()) {
            Some(id) => blob(inner, &id, self.path)?,
            None => Vec::new(),
        };
        let old = split_lines(&old_bytes);
        let (new, exact) = if file.status == ChangeStatus::TypeChanged {
            // A deletion of the old kind, then an addition of the new.
            first
                .read_against(&old, &[], false)
                .map_err(|detail| self.disagree(detail))?;
            let added = sections.get(1).unwrap_or(first);
            let new = if self.working_tree() {
                added
                    .new_side(&[])
                    .map_err(|detail| self.disagree(detail))?
                    .0
            } else {
                match file.new_id {
                    Some(id) => split_lines(&blob(inner, &id, self.path)?),
                    None => Vec::new(),
                }
            };
            (new, None)
        } else if self.working_tree() {
            let (new, reading) = first
                .new_side(&old)
                .map_err(|detail| self.disagree(detail))?;
            (new, Some(reading))
        } else {
            let new = match file.new_id.filter(|_| file.new_mode.is_some()) {
                Some(id) => split_lines(&blob(inner, &id, self.path)?),
                None => Vec::new(),
            };
            let reading = first
                .read_against(&old, &new, false)
                .map_err(|detail| self.disagree(detail))?;
            (new, Some(reading))
        };
        let new_bytes = joined(&new);
        let named = if file.new_mode.is_none() {
            None
        } else if self.working_tree() {
            let added = sections.last().unwrap_or(first);
            let computed = hash(inner, &new_bytes)?;
            match added.new_index_id() {
                Some(id) if id == computed => Some(computed),
                Some(_) => {
                    return Err(self.disagree(
                        "the lines rebuilt from git's patch are not the content git named for \
                         the working tree; the file changed while git read it"
                            .to_owned(),
                    ));
                }
                None if computed == hash(inner, &old_bytes)? => Some(computed),
                None => {
                    return Err(self.disagree(
                        "git named no object for content it printed a change to".to_owned(),
                    ));
                }
            }
        } else {
            file.new_id
        };
        let file = self.named(file, named);

        let largest = (old_bytes.len() as u64).max(new_bytes.len() as u64);
        if largest > self.ceiling {
            return Ok(Some(FileDiff {
                file,
                content: self.too_large(largest),
            }));
        }
        if !self.options.load_anyway
            && let Some(crossed) = crossed_line_limit(&old_bytes, &new_bytes, &self.options.limits)
        {
            return Ok(Some(FileDiff {
                file,
                content: DiffContent::TooLarge {
                    crossed,
                    loadable: true,
                },
            }));
        }
        if let Some(content) = lfs_pointer(&old_bytes, &new_bytes) {
            return Ok(Some(FileDiff { file, content }));
        }

        // A file added, deleted or changed in type is one change of every line, as git
        // prints it, and so is its whitespace-ignoring reading; a modification — and a
        // staged rename or copy, whose two blobs git diffed line by line — is asked again
        // with `-w` when the view wants that reading.
        let readings = match exact {
            Some(exact)
                if matches!(
                    file.status,
                    ChangeStatus::Modified | ChangeStatus::Renamed(_) | ChangeStatus::Copied(_)
                ) =>
            {
                let ignoring = if self.options.ignore_whitespace {
                    Some(self.ignoring(&old, &new, named, output)?)
                } else {
                    None
                };
                Some((exact, ignoring))
            }
            Some(_) | None => None,
        };
        Ok(Some(FileDiff {
            file,
            content: text_content(old, new, readings, self.options, self.inter_hunk_context),
        }))
    }

    /// The `-w` reading of a modification whose lines are already held: git leaving the
    /// file out, or listing it with no patch, is a change of whitespace alone.
    fn ignoring(
        &self,
        old: &[DiffLine],
        new: &[DiffLine],
        named: Option<Oid>,
        output: u64,
    ) -> Result<Reading, Error> {
        match self.ask(true, false, output)? {
            WorkingTreeAnswer::Unlisted => Ok(Reading::default()),
            WorkingTreeAnswer::PastCeiling { .. } => Err(self.disagree(
                "git's whitespace-ignoring answer was larger than its exact one".to_owned(),
            )),
            WorkingTreeAnswer::Listed { sections, .. } => {
                let Some(section) = sections.first() else {
                    return Ok(Reading::default());
                };
                if self.working_tree() && section.new_index_id() != named {
                    return Err(self.disagree(
                        "the working tree changed between git's two reads of it".to_owned(),
                    ));
                }
                section
                    .read_against(old, new, true)
                    .map_err(|detail| self.disagree(detail))
            }
        }
    }

    /// A gitlink on either side: the commit each side names, and whether git marked the
    /// working tree's checkout of it `-dirty`.
    fn submodule(&self, file: ChangedFile, sections: &[PatchText]) -> FileDiff {
        let (mut printed_old, mut printed_new, mut dirty) = (None, None, false);
        for section in sections {
            let (old, new, marked) = section.submodule_targets();
            printed_old = printed_old.or(old);
            if new.is_some() {
                printed_new = new;
                dirty = marked;
            }
        }
        let old_target = if file.old_mode == Some(FileMode::Submodule) {
            printed_old.or(file.old_id)
        } else {
            file.old_id
        };
        let new_target = match file.new_mode {
            None => None,
            Some(FileMode::Submodule) => printed_new.or(not_null(file.new_id)),
            Some(_) => {
                not_null(file.new_id).or_else(|| sections.last().and_then(PatchText::new_index_id))
            }
        };
        FileDiff {
            file: self.named(file, new_target),
            content: DiffContent::Submodule {
                old_target,
                new_target,
                dirty,
            },
        }
    }

    /// git's record with the new side's id set: a working-tree side's record carries the
    /// null id, since git does not hash the working tree for it, so it is the id of the
    /// content git read — or nothing, where Cairn did not see that content.
    fn named(&self, mut file: ChangedFile, id: Option<Oid>) -> ChangedFile {
        if file.new_mode.is_none() {
            file.new_id = None;
        } else if self.working_tree() || not_null(file.new_id).is_none() {
            file.new_id = id;
        }
        file
    }

    fn old_size(&self, file: &ChangedFile) -> Result<u64, Error> {
        match file.old_id.filter(|_| file.old_mode.is_some()) {
            Some(id) => size_of(self.repo.inner(), object_id(&id)?, self.path),
            None => Ok(0),
        }
    }

    /// The working-tree file's size on disk: what a notice can say of a side git read
    /// but printed no content of, before any filter.
    fn size_on_disk(&self) -> u64 {
        self.repo
            .workdir()
            .and_then(|workdir| {
                use std::os::unix::ffi::OsStrExt as _;
                let relative = std::ffi::OsStr::from_bytes(self.path.as_bytes());
                std::fs::symlink_metadata(workdir.join(relative)).ok()
            })
            .map_or(0, |metadata| metadata.len())
    }

    fn too_large(&self, measured: u64) -> DiffContent {
        DiffContent::TooLarge {
            crossed: SizeLimit::Bytes {
                limit: self.ceiling,
                measured,
            },
            loadable: !self.options.load_anyway
                && measured <= self.options.limits.load_anyway_bytes,
        }
    }

    /// git printed more than both sides could hold within the ceiling, so a side is past
    /// it: the measurement is the largest side known — a blob's, or the file on disk — and
    /// at least one byte past the ceiling where a filter made git's form larger than that.
    fn past_ceiling(&self, file: Option<ChangedFile>, sizes: Sizes) -> FileDiff {
        let file = match file {
            Some(file) => self.named(file, None),
            None => stand_in_file(self.path),
        };
        let on_disk = if self.working_tree() {
            self.size_on_disk()
        } else {
            0
        };
        let measured = sizes
            .largest_known()
            .max(on_disk)
            .max(self.ceiling.saturating_add(1));
        FileDiff {
            file,
            content: self.too_large(measured),
        }
    }

    fn disagree(&self, detail: String) -> Error {
        Error::ContentReadsDisagree {
            path: self.path.to_string(),
            detail,
        }
    }
}

/// Why an untracked path is not something git can be asked to diff: it is neither a
/// regular file nor a symlink. git 2.56 reads a named pipe until a writer closes it — a read
/// that never ends — git 2.30.9 and 2.32.7 print a gitlink record for one and fail, and every
/// git diffs `/dev/null` against `<dir>/null` when given a directory (reproduced on all
/// three). Decided from the path's own metadata, not followed through a symlink, which git
/// reads as its target's name; `None` where the path is a file or a symlink, or cannot be
/// read, which git then reports.
fn not_a_file(repo: &Repository, path: &RepoPath) -> Option<String> {
    use std::os::unix::ffi::OsStrExt as _;
    let workdir = repo.workdir()?;
    let metadata =
        std::fs::symlink_metadata(workdir.join(std::ffi::OsStr::from_bytes(path.as_bytes())))
            .ok()?;
    let kind = metadata.file_type();
    if kind.is_file() || kind.is_symlink() {
        None
    } else if kind.is_dir() {
        Some("a directory, which is not one file to diff".to_owned())
    } else {
        Some(
            "not a regular file or a symlink (a named pipe, a socket or a device), which git \
             does not diff"
                .to_owned(),
        )
    }
}

/// What stands in for a file git was not asked about — a conflicted path, a sparse index,
/// a bare repository: the path, with no mode or id on either side, since no one version
/// is either side.
fn stand_in(path: &RepoPath, content: DiffContent) -> FileDiff {
    FileDiff {
        file: stand_in_file(path),
        content,
    }
}

fn stand_in_file(path: &RepoPath) -> ChangedFile {
    ChangedFile {
        status: ChangeStatus::Modified,
        old_path: path.clone(),
        new_path: path.clone(),
        old_mode: None,
        new_mode: None,
        old_id: None,
        new_id: None,
    }
}

fn not_null(id: Option<Oid>) -> Option<Oid> {
    id.filter(|id| !id.as_bytes().iter().all(|byte| *byte == 0))
}

/// The index as it is on disk now, decoded whole; an empty one where there is no file yet.
fn fresh_index(repo: &gix::Repository) -> Result<gix::index::File, Error> {
    match repo.open_index() {
        Ok(index) => Ok(index),
        Err(gix::worktree::open_index::Error::IndexFile(gix::index::file::init::Error::Io(
            error,
        ))) if error.kind() == std::io::ErrorKind::NotFound => Ok(gix::index::File::from_state(
            gix::index::State::new(repo.object_hash()),
            repo.index_path(),
        )),
        Err(source) => Err(Error::DiffSetup {
            source: Box::new(source),
        }),
    }
}

/// The commit a staged diff compares the index with — `HEAD`, or the empty tree on an
/// unborn branch, as `git diff --cached` compares — and the blob `HEAD` has at `path`.
fn head_side(
    repo: &gix::Repository,
    path: &RepoPath,
) -> Result<(Oid, Option<gix::hash::ObjectId>), Error> {
    let setup = |source: Box<dyn std::error::Error + Send + Sync>| Error::DiffSetup { source };
    let peeled = repo
        .head()
        .map_err(|e| setup(Box::new(e)))?
        .try_peel_to_id()
        .map_err(|e| setup(Box::new(e)))?;
    let Some(head) = peeled else {
        let empty = gix::hash::ObjectId::empty_tree(repo.object_hash());
        return Ok((model_id(&empty)?, None));
    };
    let tree = head
        .object()
        .map_err(|e| setup(Box::new(e)))?
        .peel_to_tree()
        .map_err(|e| setup(Box::new(e)))?;
    let entry = tree
        .lookup_entry(path.as_bytes().split_str("/"))
        .map_err(|e| setup(Box::new(e)))?;
    let blob = entry
        .filter(|entry| entry.mode().is_blob_or_symlink())
        .map(|entry| entry.object_id());
    Ok((model_id(&head)?, blob))
}

/// The size of one side of a pair, from the id git named for it: zero for an absent side.
fn side_size(repo: &gix::Repository, id: &Option<Oid>, path: &RepoPath) -> Result<u64, Error> {
    match not_null(*id) {
        Some(id) => size_of(repo, object_id(&id)?, path),
        None => Ok(0),
    }
}

fn size_of(repo: &gix::Repository, id: gix::hash::ObjectId, path: &RepoPath) -> Result<u64, Error> {
    repo.find_header(id)
        .map(|header| header.size())
        .map_err(|source| Error::DiffFile {
            path: path.to_string(),
            source: Box::new(source),
        })
}

fn blob(repo: &gix::Repository, id: &Oid, path: &RepoPath) -> Result<Vec<u8>, Error> {
    repo.find_blob(object_id(id)?)
        .map(|mut blob| blob.take_data())
        .map_err(|source| Error::DiffFile {
            path: path.to_string(),
            source: Box::new(source),
        })
}

/// The blob id of `bytes`, computed and written nowhere.
fn hash(repo: &gix::Repository, bytes: &[u8]) -> Result<Oid, Error> {
    let id = gix::objs::compute_hash(repo.object_hash(), gix::objs::Kind::Blob, bytes).map_err(
        |source| Error::DiffSetup {
            source: Box::new(source),
        },
    )?;
    model_id(&id)
}

fn joined(lines: &[DiffLine]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(lines.iter().map(|line| line.bytes().len() + 1).sum());
    for line in lines {
        bytes.extend_from_slice(line.bytes());
        if line.ends_with_newline() {
            bytes.push(b'\n');
        }
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rebuilt side is hashed from exactly its bytes: a lost final newline or a lost
    /// `\r` is a different object. Caught by: joining with a newline after every line.
    #[test]
    fn a_side_is_joined_back_into_exactly_its_bytes() {
        for content in [&b""[..], b"a\n", b"a\nb", b"a\r\nb\r\n", b"\n\n"] {
            assert_eq!(joined(&split_lines(content)), content);
        }
    }

    #[test]
    fn the_null_id_names_nothing() {
        let null = Oid::parse("0000000000000000000000000000000000000000").unwrap();
        let some = Oid::parse("07da224c7ec04501dfb451be161fa962effe1dc1").unwrap();
        assert_eq!(not_null(Some(null)), None);
        assert_eq!(not_null(Some(some)), Some(some));
        assert_eq!(not_null(None), None);
    }
}
