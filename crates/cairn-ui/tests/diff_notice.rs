//! Headless tests for what stands where a file's rows would be (PRD R6.8, R6.9; criterion
//! C11, "every R6.8 state draws its notice").

use std::cell::Cell;
use std::rc::Rc;

use cairn_model::{
    ChangeStatus, ChangedFile, Context, DiffContent, DiffLine, DisplayOverlay, FileDiff, FileMode,
    Oid, RepoPath, ShownDiff, Similarity, SizeLimit, TextDiff,
};
use cairn_ui::{
    BINARY_FILE, CONFLICTED, COPIED_MODE_CHANGED, COPIED_WITHOUT_CHANGES, DiffNotice,
    DiffNoticeView, LFS_POINTER, LOAD_DIFF_CAPTION, MODE_CHANGED, NEW_SIDE, NO_CHANGES_SHOWN,
    NO_CONTENT_CHANGE, OLD_SIDE, ONLY_WHITESPACE_CHANGED, RENAMED_MODE_CHANGED,
    RENAMED_WITHOUT_CHANGES, SUBMODULE, TOO_LARGE_TO_DISPLAY, size_text,
};
use freya::prelude::*;
use freya_testing::TestingRunner;

fn file(status: ChangeStatus) -> ChangedFile {
    ChangedFile {
        status,
        old_path: RepoPath::from("old/name.bin"),
        new_path: RepoPath::from("new/name.bin"),
        old_mode: Some(FileMode::Regular),
        new_mode: Some(FileMode::Regular),
        old_id: None,
        new_id: None,
    }
}

fn shown(file: ChangedFile, content: DiffContent) -> ShownDiff {
    ShownDiff::new(FileDiff { file, content }, Context::lines(3))
}

/// Draws the notice for `shown` and returns every label it drew, and how many times its Load
/// Diff reported a press after every button was pressed.
fn draw(shown: &ShownDiff) -> (Vec<String>, usize) {
    let notice =
        DiffNotice::of(shown).unwrap_or_else(|| panic!("no notice stands in for the rows"));
    let loads = Rc::new(Cell::new(0usize));
    let counted = loads.clone();
    let (mut test, _) = TestingRunner::new(
        move || {
            let counted = counted.clone();
            rect()
                .expanded()
                .child(
                    DiffNoticeView::new(notice.clone())
                        .on_load(move |()| counted.set(counted.get() + 1)),
                )
                .into_element()
        },
        (700., 400.).into(),
        |_| {},
        1.,
    );
    test.sync_and_update();
    let labels: Vec<String> =
        test.find_many(|_, element| Label::try_downcast(element).map(|l| l.text.to_string()));
    let buttons: Vec<_> = test.find_many(|node, element| {
        Label::try_downcast(element)
            .filter(|label| label.text == LOAD_DIFF_CAPTION)
            .map(|_| node.layout().area.center())
    });
    for centre in buttons {
        test.click_cursor((f64::from(centre.x), f64::from(centre.y)));
        test.sync_and_update();
    }
    (labels, loads.get())
}

fn oid(n: u8) -> Oid {
    Oid::from_bytes(&[n; 20]).unwrap_or_else(|error| panic!("{error}"))
}

/// C11, R6.8: every state that is not text draws its own notice, in words that say what it
/// is and no other state's — both sizes for a binary, "Changes are too large to display" with
/// Load Diff only while a load is offered, the pointer text under a label for an LFS
/// pointer, git's `Subproject commit` lines (with `-dirty`) for a submodule and never
/// "binary", git's mode lines for a mode change, git's rename lines for a rename with no
/// content change, and the conflicted and unsupported states. Caught by: a state drawn with
/// another's notice (the classic: a submodule called binary), a missing size, Load Diff
/// offered past the ceiling, or one that reports nothing.
#[test]
fn every_state_that_is_not_text_draws_its_notice() {
    let (binary, _) = draw(&shown(
        file(ChangeStatus::Modified),
        DiffContent::Binary {
            old_size: 2_048,
            new_size: 1_048_576,
        },
    ));
    for wanted in [BINARY_FILE, OLD_SIDE, NEW_SIDE] {
        assert!(binary.iter().any(|l| l == wanted), "{wanted}: {binary:?}");
    }
    // KiB, labelled as such: Fork's sample (Finding 22) does not settle the base.
    assert!(
        binary.iter().any(|l| l == "2.0 KiB (2,048 bytes)"),
        "{binary:?}"
    );
    assert!(
        binary.iter().any(|l| l == "1024.0 KiB (1,048,576 bytes)"),
        "{binary:?}"
    );
    assert_eq!(size_text(1_048_576), "1024.0 KiB (1,048,576 bytes)");
    // An added binary has no old side to size.
    let (added, _) = draw(&shown(
        file(ChangeStatus::Added),
        DiffContent::Binary {
            old_size: 0,
            new_size: 10,
        },
    ));
    assert!(!added.iter().any(|l| l == OLD_SIDE), "{added:?}");

    let loadable = DiffContent::TooLarge {
        crossed: SizeLimit::Bytes {
            limit: 1_048_576,
            measured: 2_532_736,
        },
        loadable: true,
    };
    let (large, loads) = draw(&shown(file(ChangeStatus::Modified), loadable));
    assert!(large.iter().any(|l| l == TOO_LARGE_TO_DISPLAY), "{large:?}");
    assert!(
        large.iter().any(|l| l.contains("2,532,736 bytes")),
        "{large:?}"
    );
    assert_eq!(loads, 1, "Load Diff did not report its press");
    // Past the 64 MiB ceiling: the file's size and the ceiling, in MiB (the user's words,
    // 2026-10-03) — whichever limit the engine names, since the first ask names the drawing
    // limit and only a load anyway names the ceiling.
    for limit in [1_048_576, 67_108_864] {
        let (past, loads) = draw(&shown(
            file(ChangeStatus::Modified),
            DiffContent::TooLarge {
                crossed: SizeLimit::Bytes {
                    limit,
                    measured: 75_812_045,
                },
                loadable: false,
            },
        ));
        assert!(past.iter().any(|l| l == TOO_LARGE_TO_DISPLAY), "{past:?}");
        assert!(
            past.iter()
                .any(|l| l == "72.3 MiB — larger than the 64 MiB Cairn can load"),
            "{past:?}"
        );
        assert!(!past.iter().any(|l| l == LOAD_DIFF_CAPTION), "{past:?}");
        assert_eq!(loads, 0);
    }

    let pointer = "version https://git-lfs.github.com/spec/v1\noid sha256:abc\nsize 12345\n";
    let (lfs, _) = draw(&shown(
        file(ChangeStatus::Modified),
        DiffContent::LfsPointer {
            old: None,
            new: Some(pointer.to_owned()),
        },
    ));
    assert!(lfs.iter().any(|l| l == LFS_POINTER), "{lfs:?}");
    assert!(lfs.iter().any(|l| l == "size 12345"), "{lfs:?}");

    let mut submodule = file(ChangeStatus::Modified);
    submodule.old_mode = Some(FileMode::Submodule);
    submodule.new_mode = Some(FileMode::Submodule);
    let (sub, _) = draw(&shown(
        submodule,
        DiffContent::Submodule {
            old_target: Some(oid(1)),
            new_target: Some(oid(2)),
            dirty: true,
        },
    ));
    assert!(sub.iter().any(|l| l == SUBMODULE), "{sub:?}");
    assert!(
        sub.contains(&format!("-Subproject commit {}", oid(1).hex().as_str())),
        "{sub:?}"
    );
    assert!(
        sub.contains(&format!(
            "+Subproject commit {}-dirty",
            oid(2).hex().as_str()
        )),
        "{sub:?}"
    );
    assert!(
        !sub.iter().any(|l| l.to_lowercase().contains("binary")),
        "a submodule is called binary: {sub:?}"
    );

    let mut chmod = file(ChangeStatus::Modified);
    chmod.old_path = chmod.new_path.clone();
    chmod.new_mode = Some(FileMode::Executable);
    let (mode, _) = draw(&shown(chmod, DiffContent::ModeChangeOnly));
    // The user's words for a mode-only change (2026-10-03).
    assert!(mode.iter().any(|l| l == "Mode changed"), "{mode:?}");
    assert_eq!(MODE_CHANGED, "Mode changed");
    assert!(!mode.iter().any(|l| l == NO_CONTENT_CHANGE), "{mode:?}");
    assert!(mode.iter().any(|l| l == "old mode 100644"), "{mode:?}");
    assert!(mode.iter().any(|l| l == "new mode 100755"), "{mode:?}");

    let unchanged = TextDiff::new(
        vec![DiffLine::terminated("same")],
        vec![DiffLine::terminated("same")],
        Vec::new(),
    );
    let (renamed, _) = draw(&shown(
        file(ChangeStatus::Renamed(Similarity::from_percent(100))),
        DiffContent::Text {
            text: unchanged.clone(),
            overlay: DisplayOverlay::none(),
        },
    ));
    // The user's words for a rename with no content change (2026-10-03), git's lines under.
    assert!(
        renamed.iter().any(|l| l == "Renamed without changes"),
        "{renamed:?}"
    );
    assert!(
        !renamed.iter().any(|l| l == NO_CONTENT_CHANGE),
        "{renamed:?}"
    );
    for wanted in [
        "similarity index 100%",
        "rename from old/name.bin",
        "rename to new/name.bin",
    ] {
        assert!(renamed.iter().any(|l| l == wanted), "{wanted}: {renamed:?}");
    }

    // A copy with no content change, in the rename's form (Cairn's, by analogy).
    let (copied, _) = draw(&shown(
        file(ChangeStatus::Copied(Similarity::from_percent(100))),
        DiffContent::Text {
            text: unchanged.clone(),
            overlay: DisplayOverlay::none(),
        },
    ));
    assert!(
        copied.iter().any(|l| l == "Copied without changes"),
        "{copied:?}"
    );
    assert_eq!(COPIED_WITHOUT_CHANGES, "Copied without changes");
    assert!(
        !copied.iter().any(|l| l == RENAMED_WITHOUT_CHANGES),
        "a copy titled as a rename: {copied:?}"
    );
    for wanted in [
        "similarity index 100%",
        "copy from old/name.bin",
        "copy to new/name.bin",
    ] {
        assert!(copied.iter().any(|l| l == wanted), "{wanted}: {copied:?}");
    }

    // A rename or a copy whose mode moved too: the user's words (2026-10-03, the copy's by
    // analogy), git's mode lines first, as `git diff` prints them.
    for (status, title, word) in [
        (
            ChangeStatus::Renamed(Similarity::from_percent(100)),
            "Renamed, mode changed",
            "rename",
        ),
        (
            ChangeStatus::Copied(Similarity::from_percent(100)),
            "Copied, mode changed",
            "copy",
        ),
    ] {
        let mut moved = file(status);
        moved.new_mode = Some(FileMode::Executable);
        let (drawn, _) = draw(&shown(moved, DiffContent::ModeChangeOnly));
        assert!(drawn.iter().any(|l| l == title), "{title}: {drawn:?}");
        let lines: Vec<String> = drawn
            .iter()
            .skip_while(|l| *l != title)
            .skip(1)
            .cloned()
            .collect();
        assert_eq!(
            lines,
            [
                "old mode 100644".to_owned(),
                "new mode 100755".to_owned(),
                "similarity index 100%".to_owned(),
                format!("{word} from old/name.bin"),
                format!("{word} to new/name.bin"),
            ],
            "{drawn:?}"
        );
    }
    assert_eq!(RENAMED_MODE_CHANGED, "Renamed, mode changed");
    assert_eq!(COPIED_MODE_CHANGED, "Copied, mode changed");

    let (conflicted, _) = draw(&shown(
        file(ChangeStatus::Modified),
        DiffContent::Conflicted,
    ));
    let unmerged = "Unmerged path — conflicts must be resolved before a diff can be shown";
    assert!(conflicted.iter().any(|l| l == unmerged), "{conflicted:?}");
    assert_eq!(CONFLICTED, unmerged);
    let reason = "the index is a sparse index, which Cairn does not read yet";
    let (sparse, _) = draw(&shown(
        file(ChangeStatus::Modified),
        DiffContent::Unsupported {
            reason: reason.to_owned(),
        },
    ));
    assert!(sparse.iter().any(|l| l == reason), "{sparse:?}");

    let (nothing, _) = draw(&shown(
        file(ChangeStatus::Modified),
        DiffContent::Text {
            text: unchanged,
            overlay: DisplayOverlay::none(),
        },
    ));
    assert!(nothing.iter().any(|l| l == NO_CHANGES_SHOWN), "{nothing:?}");
    let respaced = TextDiff::new(
        vec![DiffLine::terminated("  a")],
        vec![DiffLine::terminated("a")],
        vec![cairn_model::ChangedRange::new(
            cairn_model::LineSpan::at(0, 1),
            cairn_model::LineSpan::at(0, 1),
        )],
    );
    let (whitespace, _) = draw(&shown(
        file(ChangeStatus::Modified),
        DiffContent::Text {
            text: respaced,
            overlay: DisplayOverlay::new(Some(Vec::new()), Vec::new()),
        },
    ));
    assert!(
        whitespace.iter().any(|l| l == ONLY_WHITESPACE_CHANGED),
        "{whitespace:?}"
    );
}

/// A text diff with rows has no notice: the rows are drawn. Caught by: a notice standing in
/// for rows that are there.
#[test]
fn a_text_diff_with_rows_has_no_notice() {
    let text = TextDiff::new(
        vec![DiffLine::terminated("a")],
        vec![DiffLine::terminated("b")],
        vec![cairn_model::ChangedRange::new(
            cairn_model::LineSpan::at(0, 1),
            cairn_model::LineSpan::at(0, 1),
        )],
    );
    let diff = shown(
        file(ChangeStatus::Renamed(Similarity::from_percent(50))),
        DiffContent::Text {
            text,
            overlay: DisplayOverlay::none(),
        },
    );
    assert_eq!(DiffNotice::of(&diff), None);
}
