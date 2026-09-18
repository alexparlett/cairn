//! C6: the content query against `git diff -U3`, git's binary rules, and R2.6's ceilings.

use std::time::Instant;

use cairn_git::{CancelSignal, ChangesRequest, ContentOptions, Repository};
use cairn_model::{
    ChangedFile, Context, DiffContent, DiffLimits, DiffLine, Oid, SizeLimit, TextDiff, UnifiedRow,
    UnifiedRows,
};

use super::repositories::{self, Repo};
use super::{ok, some};

/// One hunk as a patch spells it: the four header numbers, then the marked lines.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Hunk {
    header: (u32, u32, u32, u32),
    lines: Vec<String>,
}

const NO_NEWLINE: &str = "\\ No newline at end of file";

/// `git diff -U3 <old blob> <new blob>`, with everything above the first `@@` dropped.
///
/// Two blobs rather than two commits and a path, so that whether git would have called the
/// change a rename cannot change what is compared: the content query is handed one file's
/// two versions and this is the same two versions.
fn git_hunks(repo: &Repo, old: &str, new: &str) -> Vec<Hunk> {
    let out = repo.git(&["diff", "-U3", "--no-ext-diff", "--no-color", old, new]);
    let mut hunks: Vec<Hunk> = Vec::new();
    for line in out.split_inclusive('\n') {
        let line = line.strip_suffix('\n').unwrap_or(line);
        if let Some(rest) = line.strip_prefix("@@ ") {
            hunks.push(Hunk {
                header: parse_header(rest),
                lines: Vec::new(),
            });
            continue;
        }
        let Some(hunk) = hunks.last_mut() else {
            continue; // A header line, above the first hunk.
        };
        if line.starts_with('\\') {
            hunk.lines.push(NO_NEWLINE.to_owned());
        } else if line.starts_with([' ', '+', '-']) {
            hunk.lines.push(line.to_owned());
        }
    }
    hunks
}

/// `-a,b +c,d @@ ...`, where a count of one is written by leaving it out.
fn parse_header(rest: &str) -> (u32, u32, u32, u32) {
    let body = rest.split(" @@").next().unwrap_or_default();
    let mut sides = body.split_whitespace();
    let side = |text: &str| {
        let text = text.trim_start_matches(['-', '+']);
        let mut parts = text.split(',');
        let start: u32 = ok(parts.next().unwrap_or("0").parse(), "a line number");
        let count: u32 = ok(parts.next().unwrap_or("1").parse(), "a line count");
        (start, count)
    };
    let (old_start, old_count) = side(some(sides.next(), "an old side"));
    let (new_start, new_count) = side(some(sides.next(), "a new side"));
    (old_start, old_count, new_start, new_count)
}

/// The same shape, built from the unified projection the view draws.
fn cairn_hunks(text: &TextDiff) -> Vec<Hunk> {
    let rows = UnifiedRows::new(text, Context::lines(3));
    let mut hunks: Vec<Hunk> = Vec::new();
    for index in 0..rows.len() {
        let row = some(rows.row(index), "a row below the count");
        match row {
            UnifiedRow::Header(header) => hunks.push(Hunk {
                header: (
                    header_start(header.old),
                    header.old.len(),
                    header_start(header.new),
                    header.new.len(),
                ),
                lines: Vec::new(),
            }),
            UnifiedRow::Context { line, .. } => push(&mut hunks, ' ', line),
            UnifiedRow::Removed { line, .. } => push(&mut hunks, '-', line),
            UnifiedRow::Added { line, .. } => push(&mut hunks, '+', line),
        }
    }
    hunks
}

/// git's own rule, out of `add-patch.c`: an empty range names the line before it.
fn header_start(span: cairn_model::LineSpan) -> u32 {
    if span.is_empty() {
        span.start().index()
    } else {
        span.start().one_based()
    }
}

fn push(hunks: &mut [Hunk], marker: char, line: &DiffLine) {
    let hunk = some(hunks.last_mut(), "a row before its header");
    hunk.lines
        .push(format!("{marker}{}", String::from_utf8_lossy(line.bytes())));
    if !line.ends_with_newline() {
        hunk.lines.push(NO_NEWLINE.to_owned());
    }
}

fn text_of(content: &DiffContent) -> &TextDiff {
    match content {
        DiffContent::Text { text, .. } => text,
        other => panic!("expected a text diff, got {other:?}"),
    }
}

fn changed(repo: &Repo, commit: &str) -> Vec<ChangedFile> {
    let engine = ok(Repository::discover(repo.path()), "the fixture opens");
    ok(
        engine.changes(
            &ChangesRequest::commit(ok(Oid::parse(commit), "an id")),
            &CancelSignal::new(),
        ),
        "the changes query answers",
    )
    .files
}

/// C6's first half, over every path of every commit in the crafted history — a missing
/// final newline on each side, CRLF content, an added file, a deleted file, an empty file,
/// a one-line file and adjacent hunks all pass through here.
#[test]
fn every_crafted_file_diff_is_the_one_git_prints() {
    let repo = repositories::crafted();
    let engine = Repository::discover(repo.path()).expect("the fixture opens");
    let mut session = engine.diff_session().expect("a diff session");
    let commits: Vec<String> = repo
        .git(&["rev-list", "--reverse", "HEAD"])
        .lines()
        .map(str::to_owned)
        .collect();

    let empty = repo
        .git(&["hash-object", "-t", "blob", "/dev/null"])
        .trim()
        .to_owned();

    let mut compared = 0usize;
    for commit in &commits {
        for file in changed(&repo, commit) {
            let diff = session
                .file_diff(&file, &ContentOptions::default())
                .expect("a file diff");
            let DiffContent::Text { text, .. } = &diff.content else {
                continue; // A binary, a submodule or a mode change draws no rows at all.
            };
            let path = file.new_path.display().into_owned();
            let blob = |id: Option<Oid>| {
                id.map_or_else(|| empty.clone(), |id| id.hex().as_str().to_owned())
            };
            assert_eq!(
                cairn_hunks(text),
                git_hunks(&repo, &blob(file.old_id), &blob(file.new_id)),
                "{path} in {commit} does not read the way `git diff -U3` prints it"
            );
            compared += 1;
        }
    }
    assert!(
        compared >= 12,
        "only {compared} files were compared; the fixture lost its content"
    );
}

/// The one thing a text comparison cannot check: that the lines carry the bytes the
/// repository holds, terminator and all.
#[test]
fn both_versions_of_a_file_are_the_bytes_git_stores() {
    let repo = repositories::crafted();
    let engine = Repository::discover(repo.path()).expect("the fixture opens");
    let mut session = engine.diff_session().expect("a diff session");
    let commits: Vec<String> = repo
        .git(&["rev-list", "HEAD"])
        .lines()
        .map(str::to_owned)
        .collect();

    let mut seen_unterminated = false;
    let mut seen_crlf = false;
    for commit in &commits {
        for file in changed(&repo, commit) {
            let diff = session
                .file_diff(&file, &ContentOptions::default())
                .expect("a file diff");
            let DiffContent::Text { text, .. } = &diff.content else {
                continue;
            };
            for (id, content) in [
                (file.old_id, text.old_content()),
                (file.new_id, text.new_content()),
            ] {
                let Some(id) = id else {
                    assert!(
                        content.is_empty(),
                        "a side that does not exist has no lines"
                    );
                    continue;
                };
                let stored = repo.git(&["cat-file", "blob", id.hex().as_str()]);
                assert_eq!(
                    String::from_utf8_lossy(&content),
                    stored,
                    "{} does not hold the bytes of {id}",
                    file.new_path
                );
                seen_unterminated |= !content.is_empty() && !content.ends_with(b"\n");
                seen_crlf |= content.windows(2).any(|pair| pair == b"\r\n");
            }
        }
    }
    assert!(seen_unterminated, "no unterminated file reached the check");
    assert!(seen_crlf, "no CRLF file reached the check");
}

/// R2.5: git's three ways of calling a file binary, and the one file that is none of them.
#[test]
fn binary_detection_agrees_with_git() {
    let repo = repositories::attributes();
    let engine = Repository::discover(repo.path()).expect("the fixture opens");
    let mut session = engine.diff_session().expect("a diff session");
    let head = repo.git(&["rev-parse", "HEAD"]).trim().to_owned();

    let mut seen = Vec::new();
    for file in changed(&repo, &head) {
        let path = file.new_path.display().into_owned();
        let diff = session
            .file_diff(&file, &ContentOptions::default())
            .expect("a file diff");
        let binary_to_git = repo
            .git(&[
                "diff",
                "--no-ext-diff",
                "--no-color",
                &format!("{head}^"),
                &head,
                "--",
                &path,
            ])
            .contains("Binary files");
        let binary_to_cairn = matches!(diff.content, DiffContent::Binary { .. });
        assert_eq!(
            binary_to_cairn, binary_to_git,
            "{path}: Cairn says binary={binary_to_cairn}, git says binary={binary_to_git}"
        );
        seen.push((path, binary_to_cairn));
    }
    seen.sort();
    assert_eq!(
        seen,
        vec![
            ("flagged.dat".to_owned(), true),
            ("no-diff.txt".to_owned(), true),
            ("nul.dat".to_owned(), true),
            ("plain.txt".to_owned(), false),
            ("trap.txt".to_owned(), false),
        ],
        "the fixture must exercise every rule and one file that breaks none of them"
    );
}

/// R2.3: neither a textconv nor an external diff program runs, whatever the config says.
#[test]
fn neither_a_textconv_nor_an_external_diff_program_is_started() {
    let repo = repositories::attributes();
    let engine = Repository::discover(repo.path()).expect("the fixture opens");
    let mut session = engine.diff_session().expect("a diff session");
    let head = repo.git(&["rev-parse", "HEAD"]).trim().to_owned();

    assert!(
        !repositories::trap_ran(&repo),
        "the trap had already run before the query"
    );
    let trap = changed(&repo, &head)
        .into_iter()
        .find(|file| file.new_path.display() == "trap.txt")
        .expect("the watched file changed");
    let diff = session
        .file_diff(&trap, &ContentOptions::default())
        .expect("a file diff");
    assert!(
        !repositories::trap_ran(&repo),
        "a program ran: gix was asked to diff a path with a textconv and an external diff \
         command configured, and one of them started"
    );

    let text = text_of(&diff.content);
    assert_eq!(
        String::from_utf8_lossy(&text.new_content()),
        "watched\nby a program, changed\n",
        "the diff must be of the file's own bytes, not a program's output"
    );

    // The trap is a real program that really does leave its mark; without this the check
    // above would pass against a script that could never have run.
    repositories::run_trap(&repo);
    assert!(repositories::trap_ran(&repo));
}

/// R2.6: each ceiling fires, and says which one and what it measured.
#[test]
fn every_size_ceiling_fires_and_names_itself() {
    let repo = repositories::oversized();
    let engine = Repository::discover(repo.path()).expect("the fixture opens");
    let mut session = engine.diff_session().expect("a diff session");
    let head = repo.git(&["rev-parse", "HEAD"]).trim().to_owned();
    let limits = DiffLimits::default();

    let mut crossed = Vec::new();
    for file in changed(&repo, &head) {
        let path = file.new_path.display().into_owned();
        let diff = session
            .file_diff(&file, &ContentOptions::default())
            .expect("a file diff");
        match &diff.content {
            DiffContent::TooLarge {
                crossed: limit,
                loadable,
            } => {
                assert!(
                    loadable,
                    "{path} is under 64 MiB, so it can be loaded anyway"
                );
                crossed.push((path, *limit));
            }
            DiffContent::Text { .. } => assert_eq!(path, "small.txt", "{path} was drawn as text"),
            other => panic!("{path}: {other:?}"),
        }
    }
    crossed.sort_by(|left, right| left.0.cmp(&right.0));

    let (path, limit) = &crossed[0];
    assert_eq!(path, "line-too-long.txt");
    assert_eq!(
        *limit,
        SizeLimit::LineLength {
            limit: limits.max_line_bytes,
            measured: 2049
        }
    );
    let (path, limit) = &crossed[1];
    assert_eq!(path, "too-many-bytes.txt");
    let SizeLimit::Bytes {
        limit: fired,
        measured,
    } = limit
    else {
        panic!("{path} crossed {limit:?}, not the byte ceiling");
    };
    assert_eq!(*fired, limits.max_bytes);
    assert!(
        *measured > limits.max_bytes,
        "{measured} is not over the limit"
    );
    let (path, limit) = &crossed[2];
    assert_eq!(path, "too-many-lines.txt");
    assert_eq!(
        *limit,
        SizeLimit::Lines {
            limit: limits.max_lines,
            measured: 50_001
        }
    );
    assert_eq!(crossed.len(), 3, "{crossed:?}");
}

/// The other half of R2.6: a caller may ask for it anyway, and then gets the lines.
#[test]
fn a_file_past_a_ceiling_can_be_asked_for_anyway() {
    let repo = repositories::oversized();
    let engine = Repository::discover(repo.path()).expect("the fixture opens");
    let mut session = engine.diff_session().expect("a diff session");
    let head = repo.git(&["rev-parse", "HEAD"]).trim().to_owned();
    let anyway = ContentOptions {
        load_anyway: true,
        ..ContentOptions::default()
    };

    for file in changed(&repo, &head) {
        let path = file.new_path.display().into_owned();
        let diff = session.file_diff(&file, &anyway).expect("a file diff");
        let text = text_of(&diff.content);
        assert!(
            !text.new_lines().is_empty(),
            "{path} answered nothing when it was asked for anyway"
        );
    }
}

/// R2.6's ordering, and the product rule behind it: a file too large is refused BEFORE it
/// is read, never after the window has stalled on it.
///
/// The fixture's over-limit blob is a loose object truncated after its header, so its size
/// is readable and its content is not. Answering "too large" therefore proves the content
/// was never read — and asking for it anyway fails, which is what stops that from being a
/// claim about a file that could have been read either way.
#[test]
fn the_size_ceiling_is_decided_before_the_content_is_read() {
    let (repo, blob) = repositories::truncated_object();
    let engine = Repository::discover(repo.path()).expect("the fixture opens");
    let mut session = engine.diff_session().expect("a diff session");
    let head = repo.git(&["rev-parse", "HEAD"]).trim().to_owned();
    let file = changed(&repo, &head)
        .into_iter()
        .find(|file| file.new_path.display() == "big.txt")
        .expect("the big file changed");
    assert_eq!(
        file.new_id.map(|id| id.hex().as_str().to_owned()),
        Some(blob)
    );

    let refused = session
        .file_diff(&file, &ContentOptions::default())
        .expect("a file diff");
    assert!(
        matches!(
            refused.content,
            DiffContent::TooLarge {
                crossed: SizeLimit::Bytes { .. },
                ..
            }
        ),
        "{:?}",
        refused.content
    );

    let read_anyway = session.file_diff(
        &file,
        &ContentOptions {
            load_anyway: true,
            ..ContentOptions::default()
        },
    );
    assert!(
        read_anyway.is_err(),
        "the truncated object read back fine, so refusing it proves nothing about reading"
    );
}

/// R2.8, through the engine: the second set of ranges is display-only and beside the exact
/// one, and the lines it points at are still the original bytes.
#[test]
fn ignoring_whitespace_answers_a_second_set_of_ranges_and_keeps_the_lines() {
    let repo = Repo::new("whitespace");
    repo.write("w.txt", b"let x = 1;\nunchanged\nreal change\n");
    let base = repo.commit("seed");
    repo.write("w.txt", b"let  x=1;\nunchanged\nREAL CHANGE\n");
    let head = repo.commit("respace and edit");

    let engine = Repository::discover(repo.path()).expect("the fixture opens");
    let file = changed(&repo, head.hex().as_str())
        .into_iter()
        .next()
        .expect("one file changed");
    let _ = base;

    let plain = engine
        .file_diff(&file, &ContentOptions::default())
        .expect("a file diff");
    let DiffContent::Text { text, overlay } = &plain.content else {
        panic!("{:?}", plain.content);
    };
    assert_eq!(text.changes().len(), 2, "two lines really changed");
    assert!(
        overlay.changes_ignoring_whitespace().is_none(),
        "the second set is computed only when it is asked for"
    );

    let ignoring = engine
        .file_diff(
            &file,
            &ContentOptions {
                ignore_whitespace: true,
                ..ContentOptions::default()
            },
        )
        .expect("a file diff");
    let DiffContent::Text { text, overlay } = &ignoring.content else {
        panic!("{:?}", ignoring.content);
    };
    assert_eq!(
        text.changes().len(),
        2,
        "the exact answer must not change because a view asked for another one"
    );
    let shown = overlay
        .changes_ignoring_whitespace()
        .expect("the second set");
    assert_eq!(
        shown.len(),
        1,
        "only the real change survives -w: {shown:?}"
    );
    assert!(
        overlay.hides_a_change(text),
        "a view has to say that it is hiding one (R6.7)"
    );
    assert_eq!(
        String::from_utf8_lossy(&text.new_content()),
        "let  x=1;\nunchanged\nREAL CHANGE\n",
        "the lines drawn are the original bytes, whatever was compared"
    );
}

/// R2.7: the ranges inside a pair of lines, computed by the engine and always on.
#[test]
fn a_changed_pair_of_lines_carries_its_intra_line_ranges() {
    let repo = Repo::new("intraline");
    repo.write("i.txt", b"let value = compute(1);\n");
    repo.commit("seed");
    repo.write("i.txt", b"let value = compute(2);\n");
    let head = repo.commit("edit an argument");

    let engine = Repository::discover(repo.path()).expect("the fixture opens");
    let file = changed(&repo, head.hex().as_str())
        .into_iter()
        .next()
        .expect("one file changed");
    let diff = engine
        .file_diff(&file, &ContentOptions::default())
        .expect("a file diff");
    let DiffContent::Text { text, overlay } = &diff.content else {
        panic!("{:?}", diff.content);
    };
    let pair = overlay.highlights().first().expect("one pair of lines");
    let line = text
        .new_line(pair.added_line)
        .expect("the added line")
        .bytes();
    let marked: Vec<&[u8]> = pair
        .on_added
        .iter()
        .map(|range| &line[range.start as usize..range.end as usize])
        .collect();
    assert_eq!(marked, vec![&b"2"[..]], "only the argument moved");
}

/// A submodule is a commit id in a tree; there are no lines to read and gix refuses the
/// mode outright, so the state has to be answered before anything is asked of it.
#[test]
fn a_submodule_answers_its_commit_ids_rather_than_lines() {
    let inner = Repo::new("submodule-inner");
    inner.write("f.txt", b"one\n");
    let first = inner.commit("one");
    inner.write("f.txt", b"two\n");
    let second = inner.commit("two");

    let outer = Repo::new("submodule-outer");
    outer.config("protocol.file.allow", "always");
    outer.write("f.txt", b"outer\n");
    outer.commit("seed");
    outer.git(&[
        "-c",
        "protocol.file.allow=always",
        "submodule",
        "--quiet",
        "add",
        &inner.path().to_string_lossy(),
        "sub",
    ]);
    // `submodule add` clones the inner repository at its tip, so the submodule is moved
    // BACK first; otherwise the commit that is supposed to move it changes nothing.
    outer.git(&["-C", "sub", "checkout", "--quiet", first.hex().as_str()]);
    outer.commit("add a submodule");
    outer.git(&["-C", "sub", "checkout", "--quiet", second.hex().as_str()]);
    let head = outer.commit("move the submodule");

    let engine = Repository::discover(outer.path()).expect("the fixture opens");
    let file = changed(&outer, head.hex().as_str())
        .into_iter()
        .find(|file| file.new_path.display() == "sub")
        .expect("the submodule moved");
    let diff = engine
        .file_diff(&file, &ContentOptions::default())
        .expect("a file diff");
    assert_eq!(
        diff.content,
        DiffContent::Submodule {
            old_commit: Some(first),
            new_commit: Some(second),
        },
        "a submodule answers where it points, not lines"
    );
}

/// The whole change is the mode, so the content is not read to prove it did not move.
#[test]
fn a_mode_change_alone_answers_without_reading_the_file() {
    let repo = repositories::crafted();
    let head = repo.git(&["rev-parse", "HEAD~1"]).trim().to_owned();
    let engine = Repository::discover(repo.path()).expect("the fixture opens");
    let file = changed(&repo, &head)
        .into_iter()
        .next()
        .expect("one file changed");
    assert!(file.mode_changed(), "{file:?}");
    assert_eq!(file.old_id, file.new_id, "only the mode moved");

    let diff = engine
        .file_diff(&file, &ContentOptions::default())
        .expect("a file diff");
    assert_eq!(diff.content, DiffContent::ModeChangeOnly);
}

/// A session reuses gix's resource cache; a query that reused a stale entry would answer
/// one file's content for another.
#[test]
fn a_session_answers_the_same_as_a_query_on_its_own() {
    let repo = repositories::crafted();
    let head = repo
        .git(&["rev-list", "--max-parents=0", "HEAD"])
        .trim()
        .to_owned();
    let engine = Repository::discover(repo.path()).expect("the fixture opens");
    let files = changed(&repo, &head);
    assert!(files.len() > 2, "several files in one commit");

    let mut session = engine.diff_session().expect("a diff session");
    // Twice through the session, so a cache entry has a chance to go stale.
    for _ in 0..2 {
        for file in &files {
            let shared = session
                .file_diff(file, &ContentOptions::default())
                .expect("a file diff");
            let alone = engine
                .file_diff(file, &ContentOptions::default())
                .expect("a file diff");
            assert_eq!(shared, alone, "{} answered differently", file.new_path);
        }
    }
}

/// The timing claim R2.6 rests on, kept honest: refusing a large file must not cost what
/// reading it costs.
#[test]
fn refusing_a_large_file_costs_far_less_than_reading_it() {
    let repo = repositories::oversized();
    let engine = Repository::discover(repo.path()).expect("the fixture opens");
    let mut session = engine.diff_session().expect("a diff session");
    let head = repo.git(&["rev-parse", "HEAD"]).trim().to_owned();
    let file = changed(&repo, &head)
        .into_iter()
        .find(|file| file.new_path.display() == "too-many-bytes.txt")
        .expect("the big file changed");

    let refused = Instant::now();
    let _ = session
        .file_diff(&file, &ContentOptions::default())
        .expect("a file diff");
    let refusing = refused.elapsed();

    let read = Instant::now();
    let _ = session
        .file_diff(
            &file,
            &ContentOptions {
                load_anyway: true,
                ..ContentOptions::default()
            },
        )
        .expect("a file diff");
    let reading = read.elapsed();
    assert!(
        refusing < reading,
        "refusing took {refusing:?} and reading took {reading:?}"
    );
}
