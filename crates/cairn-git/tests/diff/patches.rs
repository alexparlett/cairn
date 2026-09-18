//! C1, C2 and C3: the patches the model emits, put through real `git apply --cached`.
//!
//! Every apply goes into a scratch index and a scratch object directory, so the repository
//! under test is only ever read. What is compared is the resulting TREE, not the patch
//! text: a patch that applies cleanly and stages the wrong bytes passes any comparison of
//! strings.

use cairn_git::{CancelSignal, ChangesRequest, ContentOptions, Repository};
use cairn_model::{
    ChangeStatus, ChangedFile, DiffContent, FileDiff, LineNumber, Oid, Patch, Selection, TextDiff,
    apply_patch, emit_patch,
};

use super::ok;
use super::repositories::{self, Repo};
use super::scratch::Scratch;

/// Everything read as text, so the emitter is exercised over the real content rather than
/// over whatever happens to sit under a display ceiling.
fn whole_file() -> ContentOptions {
    ContentOptions {
        load_anyway: true,
        ..ContentOptions::default()
    }
}

/// A file with no lines at all, which is what the emitter takes for a change that is not in
/// the file's content — a mode change, or a rename of identical bytes.
fn no_lines() -> TextDiff {
    TextDiff::new(Vec::new(), Vec::new(), Vec::new())
}

fn diffs_of(engine: &Repository, commit: &Oid) -> Vec<FileDiff> {
    let mut session = ok(engine.diff_session(), "a diff session");
    let files = ok(
        session.changes(&ChangesRequest::commit(*commit), &CancelSignal::new()),
        "the changes query answers",
    )
    .files;
    files
        .iter()
        .map(|file| ok(session.file_diff(file, &whole_file()), "a file diff"))
        .collect()
}

/// The patch for one file with every change in it selected, or `None` when the model says
/// this file has no patch at all.
fn whole_patch(diff: &FileDiff) -> Option<Patch> {
    match &diff.content {
        DiffContent::Text { text, .. } => Some(emit_patch(
            &diff.file,
            text,
            &Selection::with_every_change(text),
        )),
        // The whole change is the mode, so the headers are the whole patch.
        DiffContent::ModeChangeOnly => {
            Some(emit_patch(&diff.file, &no_lines(), &Selection::empty()))
        }
        DiffContent::Binary { .. }
        | DiffContent::TooLarge { .. }
        | DiffContent::LfsPointer { .. }
        | DiffContent::Submodule { .. }
        | DiffContent::Conflicted
        | DiffContent::Unsupported { .. } => None,
    }
}

/// Stages what a file's change did without going through a patch, for the states the model
/// deliberately has no patch for. Without this a commit holding one could not be compared
/// with its own tree at all.
fn stage_directly(scratch: &Scratch, file: &ChangedFile) {
    match (file.new_id, file.new_mode) {
        (Some(id), Some(mode)) => {
            scratch.stage(mode.octal(), id.hex().as_str(), &file.new_path.display());
        }
        _ => scratch.unstage(&file.new_path.display()),
    }
    if file.is_rename() {
        scratch.unstage(&file.old_path.display());
    }
}

/// Undoes one file's change without a patch, for the reverse leg.
fn restore_directly(scratch: &Scratch, file: &ChangedFile) {
    if file.is_copy() {
        // A copy's destination did not exist before it; its source is its own row.
        scratch.unstage(&file.new_path.display());
        return;
    }
    match (file.old_id, file.old_mode) {
        (Some(id), Some(mode)) => {
            scratch.stage(mode.octal(), id.hex().as_str(), &file.old_path.display());
        }
        _ => scratch.unstage(&file.old_path.display()),
    }
    if file.is_rename() {
        scratch.unstage(&file.new_path.display());
    }
}

fn tree_of(repo: &Repo, commit: &str) -> String {
    repo.git(&["rev-parse", &format!("{commit}^{{tree}}")])
        .trim()
        .to_owned()
}

/// The parent's tree, or the empty tree for a root commit — which is the preimage the
/// changes query compared against (L5).
fn parent_tree(repo: &Repo, commit: &str) -> String {
    match repo.try_git(&["rev-parse", &format!("{commit}^1^{{tree}}")], &[], None) {
        Ok(tree) => tree.trim().to_owned(),
        Err(_) => repo
            .git(&["hash-object", "-t", "tree", "/dev/null"])
            .trim()
            .to_owned(),
    }
}

/// C1 and C3 for one commit: forward from the parent's tree must give the commit's, and the
/// same patches in reverse from the commit's tree must give the parent's.
fn round_trips(repo: &Repo, engine: &Repository, commit: &str) -> usize {
    let id = ok(Oid::parse(commit), "an id");
    let diffs = diffs_of(engine, &id);
    if diffs.is_empty() {
        return 0;
    }

    let mut patch = Vec::new();
    let mut without_one = Vec::new();
    for diff in &diffs {
        match whole_patch(diff) {
            Some(built) => patch.extend_from_slice(built.as_bytes()),
            None => without_one.push(&diff.file),
        }
    }

    let scratch = Scratch::over(repo.path());
    let before = parent_tree(repo, commit);
    let after = tree_of(repo, commit);

    scratch.read_tree(&before);
    if !patch.is_empty()
        && let Err(complaint) = scratch.apply(&patch, false)
    {
        panic!(
            "commit {commit}: {complaint}\n--- the patch git refused ---\n{}",
            String::from_utf8_lossy(&patch)
        );
    }
    for file in &without_one {
        stage_directly(&scratch, file);
    }
    assert_eq!(
        scratch.write_tree(),
        after,
        "commit {commit} ({}): applying its patches to the parent's tree did not give its own",
        repo.git(&["log", "-1", "--format=%s", commit]).trim()
    );

    // C3, the same patches in reverse. A copy is left out of the reversed patch: `git
    // apply -R` of `copy from A / copy to B` re-creates A from B, and A is still there, so
    // it refuses — and git's OWN patch for a copy refuses the same way
    // (`gits_own_copy_patch_cannot_be_reversed_either`). Undoing a copy is removing its
    // destination, which is what is done directly here.
    let mut reversible = Vec::new();
    let mut by_hand = Vec::new();
    for diff in &diffs {
        match whole_patch(diff) {
            Some(built) if !diff.file.is_copy() => reversible.extend_from_slice(built.as_bytes()),
            _ => by_hand.push(&diff.file),
        }
    }

    scratch.read_tree(&after);
    if !reversible.is_empty()
        && let Err(complaint) = scratch.apply(&reversible, true)
    {
        panic!("commit {commit}, in reverse: {complaint}");
    }
    for file in &by_hand {
        restore_directly(&scratch, file);
    }
    assert_eq!(
        scratch.write_tree(),
        before,
        "commit {commit}: reversing its patches did not restore the parent's tree"
    );
    diffs.len()
}

fn non_merge_commits(repo: &Repo) -> Vec<String> {
    repo.git(&["rev-list", "--no-merges", "HEAD"])
        .lines()
        .map(str::to_owned)
        .collect()
}

/// C1 and C3 over the crafted fixtures, which hold the shapes a real history rarely has
/// all at once: a missing final newline on each side, CRLF, an added file, a deleted file,
/// an empty file, a rename with edits, a mode change and a type change.
#[test]
fn every_crafted_commit_round_trips_through_real_git_apply() {
    let repo = repositories::crafted();
    let engine = Repository::discover(repo.path()).expect("the fixture opens");
    let commits = non_merge_commits(&repo);
    assert!(commits.len() >= 8, "the fixture lost commits");

    let mut files = 0usize;
    for commit in &commits {
        files += round_trips(&repo, &engine, commit);
    }
    assert!(files >= 14, "only {files} files were staged");
}

/// The same, over the rename and copy fixtures, where a patch carries `rename from` and
/// `copy from` headers that no body check would ever see.
#[test]
fn every_rewrite_commit_round_trips_through_real_git_apply() {
    for config in [
        &[("diff.renames", "true")][..],
        &[("diff.renames", "copies")][..],
    ] {
        let repo = repositories::rewrites(config);
        let engine = Repository::discover(repo.path()).expect("the fixture opens");
        for commit in non_merge_commits(&repo) {
            round_trips(&repo, &engine, &commit);
        }
    }
}

/// C1 and C3 over every non-merge commit of the Cairn checkout itself: real content,
/// written by hand, in a repository nobody crafted for this test.
#[test]
fn every_commit_of_this_repository_round_trips_through_real_git_apply() {
    let engine = Repository::discover(env!("CARGO_MANIFEST_DIR")).expect("this checkout opens");
    let workdir = engine.workdir().expect("a working tree").to_owned();
    let here = Repo::borrowed(&workdir);
    let commits = non_merge_commits(&here);
    assert!(
        commits.len() >= 10,
        "only {} commits were walked",
        commits.len()
    );

    let mut files = 0usize;
    for commit in &commits {
        files += round_trips(&here, &engine, commit);
    }
    assert!(files >= 50, "only {files} files were staged");
}

/// A deterministic generator, so a failure names a seed that reproduces it.
struct Seeded(u64);

impl Seeded {
    fn next(&mut self) -> u64 {
        // Numerical Recipes' LCG: the point is repeatability, not quality.
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        self.0 >> 33
    }
}

/// The selections C2 names, in order: every line, no lines, only additions, only removals,
/// the first line of every change, the last line of every change, then seeded ones.
fn selections(text: &TextDiff, seeds: usize) -> Vec<(String, Selection)> {
    let mut out = vec![
        ("every line".to_owned(), Selection::with_every_change(text)),
        ("no lines".to_owned(), Selection::empty()),
    ];

    let mut additions = Selection::empty();
    let mut removals = Selection::empty();
    let mut first = Selection::empty();
    let mut last = Selection::empty();
    for change in text.changes() {
        for line in change.added.numbers() {
            additions.select_added(line);
        }
        for line in change.removed.numbers() {
            removals.select_removed(line);
        }
        if let Some(line) = change.removed.numbers().next() {
            first.select_removed(line);
        }
        if let Some(line) = change.added.numbers().next() {
            first.select_added(line);
        }
        if let Some(line) = change.removed.numbers().last() {
            last.select_removed(line);
        }
        if let Some(line) = change.added.numbers().last() {
            last.select_added(line);
        }
    }
    out.push(("only additions".to_owned(), additions));
    out.push(("only removals".to_owned(), removals));
    out.push(("the first line of every change".to_owned(), first));
    out.push(("the last line of every change".to_owned(), last));

    for seed in 0..seeds {
        let mut random = Seeded(seed as u64 + 1);
        let mut selection = Selection::empty();
        for change in text.changes() {
            for line in change.removed.numbers() {
                if random.next().is_multiple_of(2) {
                    selection.select_removed(line);
                }
            }
            for line in change.added.numbers() {
                if random.next().is_multiple_of(2) {
                    selection.select_added(line);
                }
            }
        }
        out.push((format!("seed {seed}"), selection));
    }
    out
}

/// Where a patch leaves the file: the path it stages, and whether the source path survives.
fn destination(file: &ChangedFile, text: &TextDiff, selection: &Selection) -> (String, bool) {
    let whole = selection.holds_every_change(text);
    let deleting = matches!(file.status, ChangeStatus::Deleted) && whole;
    (
        file.new_path.display().into_owned(),
        !deleting && !(file.is_rename() && file.old_path != file.new_path),
    )
}

/// C2: seeded selections applied by real `git apply --cached`, against what the independent
/// in-memory applier says the same patch means — and against what its HEADERS claim.
///
/// The applier discards every header line, so only the staged tree can decide whether
/// `new file mode`, `deleted file mode`, `rename from` and `old mode` are right.
#[test]
fn a_seeded_selection_stages_what_its_patch_says_it_does() {
    let repo = repositories::crafted();
    let engine = Repository::discover(repo.path()).expect("the fixture opens");
    let scratch = Scratch::over(repo.path());

    let mut checked = 0usize;
    let mut shapes: Vec<&'static str> = Vec::new();
    for commit in non_merge_commits(&repo) {
        let before = parent_tree(&repo, &commit);
        let id = Oid::parse(&commit).expect("an id");
        for diff in diffs_of(&engine, &id) {
            let DiffContent::Text { text, .. } = &diff.content else {
                continue;
            };
            shapes.push(match diff.file.status {
                ChangeStatus::Added => "added",
                ChangeStatus::Deleted => "deleted",
                ChangeStatus::Modified if diff.file.mode_changed() => "mode change",
                ChangeStatus::Modified => "modified",
                ChangeStatus::TypeChanged => "type change",
                ChangeStatus::Renamed(_) => "rename",
                ChangeStatus::Copied(_) => "copy",
            });

            for (name, selection) in selections(text, 8) {
                let patch = emit_patch(&diff.file, text, &selection);
                let where_it_lands = format!(
                    "{} in {commit}, selecting {name}",
                    diff.file.new_path.display()
                );
                if patch.is_empty() {
                    // A type change is all or nothing, so anything short of the whole of it
                    // is deliberately not a patch.
                    let atomic = matches!(diff.file.status, ChangeStatus::TypeChanged)
                        && !selection.holds_every_change(text);
                    assert!(
                        atomic
                            || selection.is_empty()
                            || !selection_touches_a_change(text, &selection),
                        "{where_it_lands}: an empty patch for a selection that holds lines"
                    );
                    continue;
                }

                scratch.read_tree(&before);
                if let Err(complaint) = scratch.apply(patch.as_bytes(), false) {
                    panic!(
                        "{where_it_lands}: {complaint}\n--- the patch ---\n{}",
                        patch.text()
                    );
                }

                let (lands_at, source_survives) = destination(&diff.file, text, &selection);
                if matches!(diff.file.status, ChangeStatus::TypeChanged) {
                    // git spells a type change as two file patches at one path, which the
                    // single-file reference applier cannot read. What the patch has to DO
                    // is stage the new kind, and that is what the index says.
                    let (mode, id) = scratch
                        .staged(&lands_at)
                        .unwrap_or_else(|| panic!("{where_it_lands}: nothing was staged"));
                    assert_eq!(
                        Some(mode.as_str()),
                        diff.file.new_mode.map(|mode| mode.octal()),
                        "{where_it_lands}: staged with the old kind's mode"
                    );
                    assert_eq!(
                        Some(id.as_str()),
                        diff.file
                            .new_id
                            .map(|id| id.hex().as_str().to_owned())
                            .as_deref(),
                        "{where_it_lands}: staged the wrong blob"
                    );
                    checked += 1;
                    continue;
                }

                let expected = apply_patch(&text.old_content(), patch.as_bytes())
                    .unwrap_or_else(|e| panic!("{where_it_lands}: the applier refused it: {e}"));
                match scratch.staged(&lands_at) {
                    Some((mode, id)) => {
                        assert_eq!(
                            scratch.blob(&id),
                            expected,
                            "{where_it_lands}: git staged different bytes than the applier gives"
                        );
                        if let Some(new_mode) = diff.file.new_mode {
                            assert_eq!(
                                mode,
                                new_mode.octal(),
                                "{where_it_lands}: staged with the wrong mode"
                            );
                        }
                    }
                    None => assert!(
                        matches!(diff.file.status, ChangeStatus::Deleted),
                        "{where_it_lands}: nothing was staged, and the file was not deleted"
                    ),
                }
                if !source_survives {
                    assert!(
                        scratch.staged(&diff.file.old_path.display()).is_none(),
                        "{where_it_lands}: the source path is still staged"
                    );
                }
                checked += 1;
            }
        }
    }

    shapes.sort_unstable();
    shapes.dedup();
    for wanted in ["added", "deleted", "modified", "rename"] {
        assert!(
            shapes.contains(&wanted),
            "no {wanted} file reached the seeded selections: {shapes:?}"
        );
    }
    assert!(checked >= 100, "only {checked} selections were applied");
}

/// Whether a selection holds any line that is actually part of a change; a selection of
/// only unchanged lines makes no patch, and that is not a bug.
fn selection_touches_a_change(text: &TextDiff, selection: &Selection) -> bool {
    text.changes().iter().any(|change| {
        change
            .removed
            .numbers()
            .any(|line| selection.holds_removed(line))
            || change
                .added
                .numbers()
                .any(|line| selection.holds_added(line))
    })
}

/// The edge C2 names by hand, because a seeded selection may never reach it: the last line
/// of a file that never ended in a newline, selected on its own.
#[test]
fn the_last_line_of_a_file_with_no_newline_stages_on_its_own() {
    let repo = Repo::new("no-eol-edge");
    repo.write("f.txt", b"one\ntwo\nthree with no newline");
    repo.commit("seed");
    repo.write("f.txt", b"ONE\ntwo\nTHREE with no newline\n");
    let head = repo.commit("both ends move");

    let engine = Repository::discover(repo.path()).expect("the fixture opens");
    let scratch = Scratch::over(repo.path());
    let before = parent_tree(&repo, head.hex().as_str());
    let diff = diffs_of(&engine, &head)
        .into_iter()
        .next()
        .expect("one file");
    let DiffContent::Text { text, .. } = &diff.content else {
        panic!("{:?}", diff.content);
    };

    let last_old = LineNumber::from_index(2);
    let last_new = LineNumber::from_index(2);
    assert!(
        !text.old_line(last_old).expect("a line").ends_with_newline(),
        "the fixture's old last line must run off the end"
    );
    assert!(
        text.new_line(last_new).expect("a line").ends_with_newline(),
        "the fixture's new last line must end cleanly"
    );

    for (name, mut selection) in [
        ("the unterminated removal alone", Selection::empty()),
        ("the terminated addition alone", Selection::empty()),
        ("both ends of the last change", Selection::empty()),
    ] {
        match name {
            "the unterminated removal alone" => selection.select_removed(last_old),
            "the terminated addition alone" => selection.select_added(last_new),
            _ => {
                selection.select_removed(last_old);
                selection.select_added(last_new);
            }
        }
        let patch = emit_patch(&diff.file, text, &selection);
        assert!(!patch.is_empty(), "{name}: nothing was emitted");
        assert!(
            patch.text().contains("\\ No newline at end of file"),
            "{name}: the marker is missing from\n{}",
            patch.text()
        );

        scratch.read_tree(&before);
        scratch
            .apply(patch.as_bytes(), false)
            .unwrap_or_else(|e| panic!("{name}: {e}\n--- the patch ---\n{}", patch.text()));
        let expected = apply_patch(&text.old_content(), patch.as_bytes())
            .unwrap_or_else(|e| panic!("{name}: the applier refused it: {e}"));
        let (_, staged) = scratch.staged("f.txt").expect("the file is staged");
        assert_eq!(
            scratch.blob(&staged),
            expected,
            "{name}: git and the applier disagree"
        );
    }
}

/// Why C3 leaves a copy out of its reverse leg: git's own patch for one cannot be reversed
/// either, so requiring it of Cairn's would be requiring more than git does.
///
/// Without this, the exclusion above would be an unchecked claim, and a real defect in the
/// copy headers could hide behind it.
#[test]
fn gits_own_copy_patch_cannot_be_reversed_either() {
    let repo = repositories::rewrites(&[("diff.renames", "copies")]);
    let head = repo.git(&["rev-parse", "HEAD"]).trim().to_owned();
    let patch = repo.git(&[
        "diff",
        "-C",
        "--no-ext-diff",
        "--no-color",
        &format!("{head}^"),
        &head,
    ]);
    assert!(
        patch.contains("copy from "),
        "git did not report a copy, so this proves nothing:\n{patch}"
    );

    let scratch = Scratch::over(repo.path());
    scratch.read_tree(&parent_tree(&repo, &head));
    scratch
        .apply(patch.as_bytes(), false)
        .expect("git's own patch applies forwards");
    assert_eq!(scratch.write_tree(), tree_of(&repo, &head));

    scratch.read_tree(&tree_of(&repo, &head));
    let refused = scratch
        .apply(patch.as_bytes(), true)
        .expect_err("git reversed its own copy patch after all; C3 can require it of Cairn's");
    assert!(
        refused.contains("already exists"),
        "git refused for another reason: {refused}"
    );
}
