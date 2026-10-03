//! C6: the content query against `git diff -U3`, git's binary rules, and R2.6's ceilings.

use std::time::Instant;

use cairn_git::{CancelSignal, ChangesRequest, ContentOptions, DiffSession, Error, Repository};
use cairn_model::{ChangedFile, DiffContent, DiffLimits, FileDiff, Oid, SizeLimit, TextDiff};

use super::repositories::{self, Repo};
use super::{ok, some};

/// A content query for one file of `commit`'s change set, as the application asks it.
pub trait Content {
    fn content(
        &mut self,
        commit: impl std::fmt::Display,
        file: &ChangedFile,
        options: &ContentOptions,
    ) -> Result<FileDiff, Error>;
}

fn request(commit: impl std::fmt::Display) -> ChangesRequest {
    ChangesRequest::commit(ok(Oid::parse(&commit.to_string()), "a commit id"))
}

impl Content for DiffSession<'_> {
    fn content(
        &mut self,
        commit: impl std::fmt::Display,
        file: &ChangedFile,
        options: &ContentOptions,
    ) -> Result<FileDiff, Error> {
        self.file_diff(
            super::git(),
            &request(commit),
            file,
            options,
            &CancelSignal::new(),
        )
    }
}

/// The same, on a session of its own.
pub trait ContentAlone {
    fn content(
        &self,
        commit: impl std::fmt::Display,
        file: &ChangedFile,
        options: &ContentOptions,
    ) -> Result<FileDiff, Error>;
}

impl ContentAlone for Repository {
    fn content(
        &self,
        commit: impl std::fmt::Display,
        file: &ChangedFile,
        options: &ContentOptions,
    ) -> Result<FileDiff, Error> {
        self.file_diff(
            super::git(),
            &request(commit),
            file,
            options,
            &CancelSignal::new(),
        )
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
            super::git(),
            &ChangesRequest::commit(ok(Oid::parse(commit), "an id")),
            &CancelSignal::new(),
        ),
        "the changes query answers",
    )
    .files
}

/// C6's first half, over every path of every commit in the crafted history — a missing
/// final newline on each side, CRLF content, an added file, a deleted file, an empty file,
/// a one-line file, a rename, a type change and adjacent hunks all pass through here — at
/// three lines of context and at one, each against the user's own `git diff`, header,
/// function context and line (`super::parity`).
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
    let mut compared = 0usize;
    for context in [3, 1] {
        for commit in &commits {
            let tally = super::parity::compare_commit(&repo, &mut session, commit, context, false);
            assert!(
                tally.divergent.is_empty(),
                "not the way `git diff -U{context}` prints it:\n{}",
                tally.divergent.join("\n")
            );
            compared += tally.files;
        }
    }
    assert!(
        compared >= 24,
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
                .content(commit, &file, &ContentOptions::default())
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

/// Each changed file of `head` as git and Cairn read it: binary or not, which must agree,
/// and for a binary file the two sizes Cairn answers, which must be the blobs' as `git
/// cat-file -s` reads them (a binary's sizes are all a view can show of it).
fn binary_verdicts(repo: &Repo, head: &str) -> Vec<(String, bool)> {
    let engine = ok(Repository::discover(repo.path()), "the fixture opens");
    let mut session = ok(engine.diff_session(), "a diff session");
    let mut seen = Vec::new();
    for file in changed(repo, head) {
        let path = file.new_path.display().into_owned();
        let diff = ok(
            session.content(head, &file, &ContentOptions::default()),
            "a file diff",
        );
        let binary_to_git = repo
            .git(&[
                "diff",
                "--no-ext-diff",
                "--no-color",
                &format!("{head}^"),
                head,
                "--",
                &path,
            ])
            .contains("Binary files");
        let binary_to_cairn = matches!(diff.content, DiffContent::Binary { .. });
        assert_eq!(
            binary_to_cairn, binary_to_git,
            "{path}: Cairn says binary={binary_to_cairn}, git says binary={binary_to_git}"
        );
        if let DiffContent::Binary { old_size, new_size } = diff.content {
            let stored = |id: Option<Oid>| -> u64 {
                id.map_or(0, |id| {
                    ok(
                        repo.git(&["cat-file", "-s", id.hex().as_str()])
                            .trim()
                            .parse(),
                        "a size",
                    )
                })
            };
            assert_eq!(
                (old_size, new_size),
                (stored(file.old_id), stored(file.new_id)),
                "{path}: the sizes of a binary file are not the blobs' (old, new)"
            );
        }
        seen.push((path, binary_to_cairn));
    }
    seen.sort();
    seen
}

/// R2.5: git's ways of calling a file binary — the `-diff` attribute, a `diff` driver
/// with `binary` set, the built-in `binary` macro, and a NUL in the first 8,000 bytes,
/// at the last byte git looks at — and the files that are none of them, a NUL at the
/// first byte git does NOT look at among them.
#[test]
fn binary_detection_agrees_with_git() {
    let repo = repositories::attributes();
    let head = repo.git(&["rev-parse", "HEAD"]).trim().to_owned();
    assert_eq!(
        binary_verdicts(&repo, &head),
        vec![
            ("flagged.dat".to_owned(), true),
            ("macro.dat".to_owned(), true),
            ("no-diff.txt".to_owned(), true),
            ("nul-at-7999.dat".to_owned(), true),
            ("nul-at-8000.txt".to_owned(), false),
            ("nul.dat".to_owned(), true),
            ("plain.txt".to_owned(), false),
            ("trap.txt".to_owned(), false),
        ],
        "the fixture must exercise every rule and files that break none of them"
    );
}

/// R2.5's last rule: `core.bigFileThreshold`, past which git calls a file binary without
/// reading it, beside a file under it that is text.
#[test]
fn a_file_past_big_file_threshold_is_binary_as_git_says() {
    let repo = repositories::big_file_threshold();
    let head = repo.git(&["rev-parse", "HEAD"]).trim().to_owned();
    assert_eq!(
        binary_verdicts(&repo, &head),
        vec![
            ("past-the-threshold.txt".to_owned(), true),
            ("under-the-threshold.txt".to_owned(), false),
        ],
        "the threshold must be crossed by one file and not the other"
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
        .content(&head, &trap, &ContentOptions::default())
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
            .content(&head, &file, &ContentOptions::default())
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
        let diff = session.content(&head, &file, &anyway).expect("a file diff");
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
        .content(&head, &file, &ContentOptions::default())
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

    let read_anyway = session.content(
        &head,
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

/// R2.6's last ceiling: past `load_anyway_bytes` nothing is offered, and a caller asking
/// anyway is refused before the content is read, told which ceiling it crossed. Over the
/// truncated object, so an answer that read the content fails rather than passing, and
/// with the ceiling set either side of the object's size. Caught by: `loadable` answered
/// true past the ceiling, or the ceiling not applied to a load anyway.
#[test]
fn past_the_load_anyway_ceiling_nothing_is_offered_or_read() {
    let (repo, blob) = repositories::truncated_object();
    let engine = Repository::discover(repo.path()).expect("the fixture opens");
    let mut session = engine.diff_session().expect("a diff session");
    let head = repo.git(&["rev-parse", "HEAD"]).trim().to_owned();
    let file = changed(&repo, &head)
        .into_iter()
        .find(|file| file.new_path.display() == "big.txt")
        .expect("the big file changed");
    let size: u64 = ok(
        repo.git(&["cat-file", "-s", &blob]).trim().parse(),
        "a size",
    );
    let limits = DiffLimits {
        load_anyway_bytes: size - 1,
        ..DiffLimits::default()
    };
    assert!(
        limits.max_bytes < limits.load_anyway_bytes,
        "the object must be past both ceilings, the drawing one first"
    );
    let ask = |load_anyway: bool, limits: DiffLimits| ContentOptions {
        limits,
        load_anyway,
        ..ContentOptions::default()
    };

    let offered = session
        .content(&head, &file, &ask(false, limits))
        .expect("a file diff");
    assert_eq!(
        offered.content,
        DiffContent::TooLarge {
            crossed: SizeLimit::Bytes {
                limit: limits.max_bytes,
                measured: size
            },
            loadable: false,
        },
        "past the load-anyway ceiling, loading anyway is not offered"
    );
    let anyway = session
        .content(&head, &file, &ask(true, limits))
        .expect("asking anyway past the ceiling is answered from the header, never by reading");
    assert_eq!(
        anyway.content,
        DiffContent::TooLarge {
            crossed: SizeLimit::Bytes {
                limit: limits.load_anyway_bytes,
                measured: size
            },
            loadable: false,
        },
        "asking anyway past the load-anyway ceiling names that ceiling"
    );

    // The control: at exactly the object's size the same file is offered.
    let at_size = DiffLimits {
        load_anyway_bytes: size,
        ..DiffLimits::default()
    };
    let offered = session
        .content(&head, &file, &ask(false, at_size))
        .expect("a file diff");
    assert_eq!(
        offered.content,
        DiffContent::TooLarge {
            crossed: SizeLimit::Bytes {
                limit: at_size.max_bytes,
                measured: size
            },
            loadable: true,
        },
        "a file exactly at the load-anyway ceiling can be loaded anyway"
    );
}

/// R2.6's byte ceiling at its edge: a file exactly `max_bytes` long is inside it, and one
/// byte less of ceiling refuses it. Caught by: `>` turned `>=`.
#[test]
fn a_file_exactly_at_the_byte_ceiling_is_drawn() {
    let repo = Repo::new("exact-bytes");
    repo.write("f.txt", b"one\n");
    repo.commit("seed");
    repo.write("f.txt", b"one\ntwo\nthree\n");
    let head = repo.commit("grow");
    let file = changed(&repo, head.hex().as_str())
        .into_iter()
        .next()
        .expect("one file changed");
    let new_id = some(file.new_id, "the new side");
    let size: u64 = ok(
        repo.git(&["cat-file", "-s", new_id.hex().as_str()])
            .trim()
            .parse(),
        "a size",
    );
    let engine = Repository::discover(repo.path()).expect("the fixture opens");
    let at = |max_bytes: u64| ContentOptions {
        limits: DiffLimits {
            max_bytes,
            ..DiffLimits::default()
        },
        ..ContentOptions::default()
    };

    let drawn = engine.content(head, &file, &at(size)).expect("a file diff");
    assert!(
        matches!(drawn.content, DiffContent::Text { .. }),
        "a file exactly at the ceiling was refused: {:?}",
        drawn.content
    );
    let refused = engine
        .content(head, &file, &at(size - 1))
        .expect("a file diff");
    assert_eq!(
        refused.content,
        DiffContent::TooLarge {
            crossed: SizeLimit::Bytes {
                limit: size - 1,
                measured: size
            },
            loadable: true,
        },
        "one byte past the ceiling is refused"
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
        .content(head, &file, &ContentOptions::default())
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
        .content(
            head,
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
        .content(head, &file, &ContentOptions::default())
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
        .content(head, &file, &ContentOptions::default())
        .expect("a file diff");
    assert_eq!(
        diff.content,
        DiffContent::Submodule {
            old_target: Some(first),
            new_target: Some(second),
            dirty: false,
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
        .content(&head, &file, &ContentOptions::default())
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
                .content(&head, file, &ContentOptions::default())
                .expect("a file diff");
            let alone = engine
                .content(&head, file, &ContentOptions::default())
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
        .content(&head, &file, &ContentOptions::default())
        .expect("a file diff");
    let refusing = refused.elapsed();

    let read = Instant::now();
    let _ = session
        .content(
            &head,
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

/// Every file under `dir`, by path, with its bytes.
fn snapshot(dir: &std::path::Path) -> std::collections::BTreeMap<std::path::PathBuf, Vec<u8>> {
    let mut files = std::collections::BTreeMap::new();
    let mut pending = vec![dir.to_owned()];
    while let Some(next) = pending.pop() {
        for entry in ok(std::fs::read_dir(&next), "a directory reads") {
            let path = ok(entry, "an entry").path();
            if path.is_dir() {
                pending.push(path);
            } else {
                files.insert(path.clone(), ok(std::fs::read(&path), "a file reads"));
            }
        }
    }
    files
}

/// The content query's reads write nothing and run nothing the user configured but the
/// repository's `core.fsmonitor`, which every read with a working tree runs as the user's
/// own `git diff` runs it (the user's decision, QA round 3): with a textconv that caches
/// (`cachetextconv`, which `--textconv` would write a notes ref for), an external diff
/// (`diff.external`) and a driver's `command`, a clean and a smudge filter on every text
/// file, a driver naming its own algorithm (so `git check-attr` runs, from git 2.40, which
/// the command log is shown to record), `core.fsmonitor` naming a program, and the working
/// tree both stat-dirty and content-dirty, every file's content query — exact and ignoring
/// whitespace, alone and through Expand All — leaves the git directory byte-identical, the
/// fsmonitor program's mark the only one behind. The programs are then run by hand, so
/// their not running decides something. Which flags the reads pass is pinned on their
/// argument vectors (`crates/cairn-git/src/reads/patches.rs`,
/// `crates/cairn-git/src/reads/attributes.rs`). Caught by: `--textconv` or `--ext-diff` on
/// a read, porcelain `git diff` in place of `diff-tree` (which refreshes the index), a read
/// of the working tree through a filter, or the driver's `check-attr` not run.
#[test]
fn the_content_query_writes_nothing_and_runs_nothing() {
    let repo = repositories::attributes();
    let mark = repo.path().join("mark.sh");
    let marked = repo.path().join("filter-ran");
    std::fs::write(
        &mark,
        format!("#!/bin/sh\n: > '{}'\ncat\n", marked.display()),
    )
    .unwrap_or_else(|e| panic!("writing the filter: {e}"));
    repo.config("diff.trap.cachetextconv", "true");
    repo.config("diff.trap.algorithm", "patience");
    repo.config(
        "diff.external",
        &format!("sh {}", repo.path().join("trap.sh").display()),
    );
    repo.config("filter.mark.clean", &format!("sh {}", mark.display()));
    repo.config("filter.mark.smudge", &format!("sh {}", mark.display()));
    repo.config("filter.mark.required", "true");
    // The one program a read may run: it records that it ran and answers nothing, so git
    // scans the working tree itself.
    let monitor = repo.path().join("monitor.sh");
    let monitored = repo.path().join("fsmonitor-ran");
    std::fs::write(
        &monitor,
        format!(
            "#!/bin/sh\necho \"$@\" >> '{}'\nexit 1\n",
            monitored.display()
        ),
    )
    .unwrap_or_else(|e| panic!("writing the fsmonitor hook: {e}"));
    repo.config("core.fsmonitor", &format!("sh {}", monitor.display()));
    // Content-dirty: the attributes themselves, now naming the filter, and a file.
    repo.write(
        ".gitattributes",
        b"no-diff.txt -diff\nflagged.dat diff=flagged\ntrap.txt diff=trap\nmacro.dat binary\n\
          *.txt filter=mark\n",
    );
    repo.write("plain.txt", b"ordinary, changed, and changed again\n");
    // Stat-dirty: the same bytes, a new mtime, so a refresh would have something to record.
    std::thread::sleep(std::time::Duration::from_millis(20));
    repo.write("trap.txt", b"watched\nby a program, changed\n");
    let before = snapshot(&repo.path().join(".git"));

    let shared = ok(
        cairn_git::SharedRepository::discover(repo.path()),
        "the fixture opens",
    );
    let engine = shared.to_worker();
    let mut session = ok(engine.diff_session(), "a diff session");
    let head = repo.rev("HEAD");
    let request = ChangesRequest::commit(head);
    let set = ok(
        session.changes(super::git(), &request, &CancelSignal::new()),
        "the changes query answers",
    );
    let mut text_files = 0usize;
    for ignore_whitespace in [false, true] {
        let options = ContentOptions {
            ignore_whitespace,
            ..ContentOptions::default()
        };
        for file in &set.files {
            let diff = ok(
                session.file_diff(super::git(), &request, file, &options, &CancelSignal::new()),
                "a file diff",
            );
            text_files += usize::from(matches!(diff.content, DiffContent::Text { .. }));
        }
        let _ = ok(
            session.file_diffs(super::git(), &request, &set, &options, &CancelSignal::new()),
            "Expand All answers",
        );
    }
    assert!(text_files >= 6, "only {text_files} text files were read");

    assert_eq!(
        snapshot(&repo.path().join(".git")),
        before,
        "the git directory changed under a read"
    );
    assert!(
        !repo.path().join(".git/refs/notes").exists(),
        "a textconv cache was written"
    );
    assert!(
        !repositories::trap_ran(&repo),
        "a textconv or diff program ran"
    );
    assert!(!marked.exists(), "a clean or smudge filter ran");
    assert!(
        monitored.exists(),
        "the repository's core.fsmonitor did not run, where the user's own git diff runs it"
    );
    let log = shared.command_log();
    let ran = |verb: &str| {
        log.iter()
            .filter(|record| record.arguments.iter().any(|argument| argument == verb))
            .count()
    };
    assert!(ran("diff-tree") > 0, "no diff-tree ran: {log:?}");
    let drivers_read = super::git().version()
        >= cairn_git::ops::GitVersion {
            major: 2,
            minor: 40,
            patch: 0,
        };
    if drivers_read {
        assert!(
            ran("check-attr") > 0,
            "the driver's check-attr never ran, so its running nothing decides nothing: \
             {log:?}"
        );
    } else {
        assert_eq!(
            ran("check-attr"),
            0,
            "check-attr ran for a git that reads no driver algorithm: {log:?}"
        );
    }

    repositories::run_trap(&repo);
    assert!(repositories::trap_ran(&repo), "the trap cannot run at all");
    let status = std::process::Command::new("sh")
        .arg(&mark)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .status()
        .unwrap_or_else(|e| panic!("running the filter by hand: {e}"));
    assert!(
        status.success() && marked.exists(),
        "the filter cannot run at all"
    );
}

/// From git 2.40 a diff driver's algorithm beats `diff.algorithm` for the paths whose
/// attributes name the driver, and those paths are git's to say: here the working tree's
/// `.gitattributes`, changed since the commit, hands the driver every file under
/// `random/`, and each still reads as `git diff` shows it — which, under the driver's
/// patience against a configured histogram, the fixture is shown to tell apart. Before
/// 2.40 git ignores the key, and so does the query, starting no `check-attr`. Caught by:
/// reading the attributes from the commit rather than where git reads them, or passing
/// `--diff-algorithm` for a path whose driver names its own.
#[test]
fn a_drivers_algorithm_applies_where_git_says_the_driver_does() {
    let repo = repositories::discriminating(&[
        ("diff.algorithm", "histogram"),
        ("diff.drv.algorithm", "patience"),
    ]);
    repo.write(".gitattributes", b"random/** diff=drv\n");
    let head = repo.git(&["rev-parse", "HEAD"]).trim().to_owned();
    let shared = ok(
        cairn_git::SharedRepository::discover(repo.path()),
        "the fixture opens",
    );
    let engine = shared.to_worker();
    let mut session = ok(engine.diff_session(), "a diff session");
    let tally = super::parity::compare_commit(&repo, &mut session, &head, 3, false);
    assert!(
        tally.divergent.is_empty(),
        "{} files differ from git diff:\n{}",
        tally.divergent.len(),
        tally.divergent.join("\n")
    );
    let attribute_reads = shared
        .command_log()
        .iter()
        .filter(|record| {
            record
                .arguments
                .iter()
                .any(|argument| argument == "check-attr")
        })
        .count();
    let drivers_read = super::git().version()
        >= cairn_git::ops::GitVersion {
            major: 2,
            minor: 40,
            patch: 0,
        };
    if drivers_read {
        assert!(attribute_reads > 0, "no git check-attr ran");
        let patience = repo.git(&[
            "-c",
            "diff.algorithm=patience",
            "diff",
            "HEAD^",
            "HEAD",
            "--",
            "random",
        ]);
        let histogram = repo.git(&[
            "-c",
            "diff.drv.algorithm=histogram",
            "diff",
            "HEAD^",
            "HEAD",
            "--",
            "random",
        ]);
        assert_ne!(
            patience, histogram,
            "the fixture does not tell the driver's algorithm from diff.algorithm"
        );
    } else {
        assert_eq!(
            attribute_reads, 0,
            "check-attr ran for a git that reads no driver algorithm"
        );
    }
}
