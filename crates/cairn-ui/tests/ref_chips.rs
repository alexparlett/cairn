//! C7's row half (refs-and-status R5): which chips a row draws, in what order, compacted and
//! clipped, and how each kind is drawn — its glyph, the current branch's check mark, the bold
//! `HEAD` subject — headless.

use std::any::Any;

use cairn_model::{
    CommitSummary, GraphRow, HeadState, History, Label as RefLabel, Lane, Oid, PagedCommit, Ref,
    RefKind, RefName, RefTarget, RefsSnapshot, RowEdges, RowsPage, Upstream,
};
use cairn_ui::{
    CHIP_HEIGHT, Chip, ChipKind, CommitRow, GLYPH_SIZE, ROW_HEIGHT, chip_element, label_room,
    min_width, row_chips,
};
use freya::prelude::*;
use freya_testing::TestingRunner;

fn oid(n: u8) -> Oid {
    Oid::from_bytes(&[n; 20]).unwrap_or_else(|_| unreachable!("20 bytes is a SHA-1"))
}

/// A history of one row, `HEAD`'s when `head`, labelled by `names` (full names; kinds read
/// from their namespace; `current` the branch `HEAD` is on).
fn one_row(names: &[&str], current: Option<&str>, head: bool) -> History {
    let labels: Vec<RefLabel<'_>> = names
        .iter()
        .map(|name| RefLabel {
            name,
            kind: kind_of(name),
            current: current == Some(*name),
        })
        .collect();
    let mut page = RowsPage::new();
    page.push_labelled(
        GraphRow::new(oid(1), Lane::new(0), Vec::new()),
        PagedCommit {
            parents: 1,
            subject: "subject",
            author: "Ada",
            author_time: 0,
        },
        head,
        &labels,
    );
    let mut history = History::new();
    history.append(page).unwrap_or_else(|full| panic!("{full}"));
    history
}

fn kind_of(name: &str) -> RefKind {
    if name.starts_with("refs/heads/") {
        RefKind::LocalBranch
    } else if name.starts_with("refs/remotes/") {
        RefKind::RemoteTracking
    } else {
        RefKind::Tag
    }
}

/// A snapshot listing `refs` (full name, commit, upstream), `HEAD` on `head`.
fn snapshot(refs: &[(&str, u8, Option<&str>)], head: HeadState) -> RefsSnapshot {
    let mut listed: Vec<Ref> = refs
        .iter()
        .map(|(name, commit, upstream)| Ref {
            name: RefName::new(*name),
            kind: kind_of(name),
            target: RefTarget::Commit(oid(*commit)),
            symbolic: None,
            upstream: upstream.map(|upstream| {
                let at = refs.iter().find(|(other, ..)| other == &upstream);
                match at {
                    Some((_, commit, _)) => Upstream::Exists {
                        name: RefName::new(upstream),
                        commit: Some(oid(*commit)),
                    },
                    None => Upstream::Gone {
                        name: RefName::new(upstream),
                    },
                }
            }),
        })
        .collect();
    listed.sort_by(|one, other| one.name.as_str().cmp(other.name.as_str()));
    RefsSnapshot {
        refs: listed,
        head,
        stashes: Vec::new(),
        unreadable: 0,
    }
}

fn branch(text: &str, current: bool, tracked: bool) -> Chip {
    Chip {
        kind: ChipKind::Branch { current, tracked },
        text: text.to_owned(),
    }
}

fn chip(kind: ChipKind, text: &str) -> Chip {
    Chip {
        kind,
        text: text.to_owned(),
    }
}

/// Chips with all the room they could want.
fn laid(history: &History, refs: Option<&RefsSnapshot>) -> Vec<Chip> {
    let row = history.row(0).unwrap_or_else(|| panic!("no row"));
    row_chips(row.labels(), refs, f32::INFINITY)
}

/// R5.2, the QA brief's three cases: a local branch and its upstream at one commit are one
/// chip with the remote glyph; a second remote's ref at that commit keeps its own chip; an
/// upstream at a different commit is not folded. Caught by: folding any remote of the same
/// name, folding the upstream wherever it is, or drawing the folded upstream's chip too.
#[test]
fn a_branch_and_its_upstream_at_one_commit_are_one_chip_and_nothing_else_is_folded() {
    let refs = snapshot(
        &[
            ("refs/heads/main", 1, Some("refs/remotes/origin/main")),
            ("refs/heads/topic", 1, Some("refs/remotes/origin/topic")),
            ("refs/remotes/origin/main", 1, None),
            ("refs/remotes/mike/main", 1, None),
            ("refs/remotes/origin/topic", 2, None),
        ],
        HeadState::Branch(RefName::new("refs/heads/main")),
    );
    let history = one_row(
        &[
            "refs/heads/main",
            "refs/heads/topic",
            "refs/remotes/mike/main",
            "refs/remotes/origin/main",
        ],
        Some("refs/heads/main"),
        true,
    );
    assert_eq!(
        laid(&history, Some(&refs)),
        [
            branch("main", true, true),
            branch("topic", false, false),
            chip(ChipKind::Remote, "mike/main"),
        ]
    );
}

/// A branch whose upstream is another local branch (`remote = .`) is not compacted: Fork
/// folds a remote-tracking ref alone. And with no snapshot yet, nothing is folded.
#[test]
fn only_a_remote_tracking_upstream_is_folded_and_only_with_a_snapshot() {
    let refs = snapshot(
        &[
            ("refs/heads/feature", 1, Some("refs/heads/main")),
            ("refs/heads/main", 1, Some("refs/remotes/origin/main")),
            ("refs/remotes/origin/main", 1, None),
        ],
        HeadState::Detached(oid(9)),
    );
    let history = one_row(
        &[
            "refs/heads/feature",
            "refs/heads/main",
            "refs/remotes/origin/main",
        ],
        None,
        false,
    );
    assert_eq!(
        laid(&history, Some(&refs)),
        [branch("feature", false, false), branch("main", false, true)]
    );
    assert_eq!(
        laid(&history, None),
        [
            branch("feature", false, false),
            branch("main", false, false),
            chip(ChipKind::Remote, "origin/main"),
        ]
    );
}

/// The current branch leads, then the row's other refs in the snapshot's order — local
/// branches, remote-tracking refs, tags — each kind its own chip. Caught by: the current
/// branch left in name order, its check mark lost, or a kind drawn as another.
#[test]
fn the_current_branch_leads_then_branches_remotes_and_tags_in_order() {
    let refs = snapshot(
        &[
            ("refs/heads/alpha", 1, None),
            ("refs/heads/zeta", 1, None),
            ("refs/remotes/origin/x", 1, None),
            ("refs/tags/v1.0", 1, None),
        ],
        HeadState::Branch(RefName::new("refs/heads/zeta")),
    );
    let history = one_row(
        &[
            "refs/tags/v1.0",
            "refs/remotes/origin/x",
            "refs/heads/zeta",
            "refs/heads/alpha",
        ],
        Some("refs/heads/zeta"),
        true,
    );
    assert_eq!(
        laid(&history, Some(&refs)),
        [
            branch("zeta", true, false),
            branch("alpha", false, false),
            chip(ChipKind::Remote, "origin/x"),
            chip(ChipKind::Tag, "v1.0"),
        ]
    );
}

/// R5.3, and the uncapped labels' bound (phase 05 QA, RR3): chips stop being built once the
/// room is spent — the one crossing the edge is built, none after it — so ten thousand refs
/// on one commit build a column's worth. Caught by: building every chip, stopping before
/// the edge, or counting the rest as "+N".
#[test]
fn chips_stop_being_built_at_the_rooms_edge_however_many_refs_the_commit_has() {
    let names: Vec<String> = (0..10_000).map(|n| format!("refs/tags/t{n:05}")).collect();
    let borrowed: Vec<&str> = names.iter().map(String::as_str).collect();
    let history = one_row(&borrowed, None, false);
    let row = history.row(0).unwrap_or_else(|| panic!("no row"));
    let room = 500.;
    let chips = row_chips(row.labels(), None, room);

    let spent: Vec<f32> = chips
        .iter()
        .scan(0., |used, chip| {
            *used += min_width(chip) + cairn_ui::CHIP_GAP;
            Some(*used)
        })
        .collect();
    let (last, before) = match spent.as_slice() {
        [.., before, last] => (*last, *before),
        _ => panic!("fewer than two chips built: {chips:?}"),
    };
    assert!(
        last >= room,
        "the chips built stop short of the edge: {last} of {room}"
    );
    assert!(before < room, "a chip was built wholly past the edge");
    assert!(chips.len() < 100, "{} chips built for 500 px", chips.len());
    assert!(
        chips.iter().all(|chip| !chip.text.starts_with('+')),
        "a count was drawn in place of the clipped chips"
    );
    assert_eq!(chips.first().map(|chip| chip.text.as_str()), Some("t00000"));
}

/// A stash's chip is `stash@{n}`, with the box (R5.4).
#[test]
fn a_stash_chip_names_its_entry() {
    let stash = Chip::stash(3);
    assert_eq!(stash.text, "stash@{3}");
    assert_eq!(stash.kind, ChipKind::Stash);
}

const WIDTH: f32 = 900.;

#[derive(Clone)]
struct Drawn {
    chips: Vec<Chip>,
    head: bool,
}

fn row_app() -> impl IntoElement {
    let Drawn { chips, head } = use_consume::<Drawn>();
    rect().width(Size::fill()).child(
        CommitRow::new(
            CommitSummary {
                id: oid(1),
                parent_count: 1,
                summary: "the subject".to_owned(),
                author_name: "Ada Lovelace".to_owned(),
                author_time: 0,
            },
            RowEdges {
                lane: Lane::new(0),
                edges: Vec::new(),
            },
            1,
        )
        .chips(chips)
        .head(head),
    )
}

fn draw(chips: Vec<Chip>, head: bool, width: f32) -> TestingRunner {
    let (mut test, ()) = TestingRunner::new(
        row_app,
        (width, 100.).into(),
        move |runner| {
            runner.provide_root_context(|| Drawn { chips, head });
        },
        1.,
    );
    test.sync_and_update();
    test
}

/// `(left, right)` of the label reading `text`.
fn label_at(test: &TestingRunner, text: &str) -> (f32, f32) {
    test.find(|node, element| {
        Label::try_downcast(element)
            .filter(|label| label.text == text)
            .map(|_| {
                let area = node.layout().area;
                (area.min_x(), area.max_x())
            })
    })
    .unwrap_or_else(|| panic!("no label reads {text:?}"))
}

/// Whether the label reading `text`, inside the row's clipped column, is drawn: the toolkit's
/// own visibility, which reads the clipping ancestors.
fn shown(test: &TestingRunner, text: &str) -> bool {
    test.find(|node, element| {
        Label::try_downcast(element)
            .filter(|label| label.text == text)
            .map(|_| node.is_visible())
    })
    .unwrap_or_else(|| panic!("no label reads {text:?}"))
}

/// The left edge of every glyph canvas (the graph's is wider and left out), in order.
fn glyphs(test: &TestingRunner) -> Vec<f32> {
    let mut found = test.find_many(|node, element| {
        (element as &dyn Any)
            .downcast_ref::<CanvasElement>()
            .map(|_| node.layout().area)
            .filter(|area| area.width() == GLYPH_SIZE)
            .map(|area| area.min_x())
    });
    found.sort_by(f32::total_cmp);
    found
}

/// C7, R5.1 and R5.5: each kind of chip draws its own glyph before its name — the current
/// branch its check mark, a compact branch, a remote-tracking ref, a tag and a stash a glyph
/// in a cap — and a plain branch none. Caught by: a cap dropped from a kind, a check mark on
/// a branch that is not current, or a glyph after its name.
#[test]
fn each_kind_of_chip_draws_its_glyph_before_its_name_and_a_plain_branch_none() {
    let chips = vec![
        branch("main", true, false),
        branch("plain", false, false),
        branch("synced", false, true),
        chip(ChipKind::Remote, "origin/x"),
        chip(ChipKind::Tag, "v1.0"),
        chip(ChipKind::Stash, "stash@{0}"),
    ];
    let test = draw(chips, false, 1_600.);
    let glyphs = glyphs(&test);
    assert_eq!(
        glyphs.len(),
        5,
        "one glyph for each chip but the plain one: {glyphs:?}"
    );
    let names = ["main", "plain", "synced", "origin/x", "v1.0", "stash@{0}"]
        .map(|text| label_at(&test, text));
    // Each glyph sits after the name before it and before its own chip's name.
    let owners = [0, 2, 3, 4, 5];
    for (glyph, owner) in glyphs.iter().zip(owners) {
        assert!(
            *glyph < names[owner].0,
            "a glyph is not before {owner}'s name"
        );
        if owner > 0 {
            assert!(
                *glyph > names[owner - 1].1,
                "a glyph is not after the name before it"
            );
        }
    }
}

/// The `HEAD` row's subject is bold, every other row's not (R5.1). Caught by: the flag
/// ignored, or every row bold.
#[test]
fn the_head_rows_subject_is_bold_and_no_other() {
    let weight = |head: bool| {
        draw(Vec::new(), head, WIDTH)
            .find(|_, element| {
                Label::try_downcast(element)
                    .filter(|label| label.text == "the subject")
                    .map(|label| label.text_style_data.font_weight)
            })
            .unwrap_or_else(|| panic!("no subject"))
    };
    assert_eq!(weight(true), Some(FontWeight::BOLD));
    assert_ne!(weight(false), Some(FontWeight::BOLD));
}

/// The QA brief: a row with twenty refs clips rather than wraps, and keeps its height — the
/// chips past the column's edge are not visible, the subject is pushed out, and the author
/// column stays where it is. Caught by: no clip on the column, a wrapping chip row, or the
/// chips pushing the fixed columns over.
#[test]
fn twenty_refs_are_clipped_at_the_columns_edge_and_the_row_keeps_its_height() {
    let width = 700.;
    let chips: Vec<Chip> = (0..20)
        .map(|n| branch(&format!("feature/branch-{n:02}"), false, false))
        .collect();
    let plain = draw(Vec::new(), false, width);
    let author = label_at(&plain, "Ada Lovelace");

    let test = draw(chips, false, width);
    assert_eq!(
        label_at(&test, "Ada Lovelace"),
        author,
        "the author column moved"
    );
    let column_edge = author.0 - cairn_ui::COLUMN_GAP;
    assert!(
        shown(&test, "feature/branch-00"),
        "the first chip is not drawn"
    );
    assert!(
        !shown(&test, "feature/branch-19"),
        "the twentieth chip is visible: nothing clipped it"
    );
    let last = label_at(&test, "feature/branch-19");
    assert!(
        last.0 > column_edge,
        "the chips wrapped back inside the column"
    );
    assert!(
        !shown(&test, "the subject"),
        "the subject was not pushed out of the column"
    );

    let heights = test.find_many(|node, element| {
        Rect::try_downcast(element)
            .map(|_| node.layout().area)
            .filter(|area| area.width() == width && area.min_y() == 0.)
            .map(|area| area.height())
    });
    // The window's own root is the window's height; every rect the row spans is the row's.
    assert!(heights.len() >= 3, "{heights:?}");
    assert!(
        heights.iter().skip(1).all(|height| *height == ROW_HEIGHT),
        "the row grew: {heights:?}"
    );
    let chip_tops = test.find_many(|node, element| {
        Rect::try_downcast(element)
            .map(|_| node.layout().area)
            .filter(|area| area.height() == CHIP_HEIGHT)
            .map(|area| area.min_y())
    });
    assert!(chip_tops.len() >= 20, "{} chips drawn", chip_tops.len());
    assert!(
        chip_tops.windows(2).all(|pair| pair[0] == pair[1]),
        "the chips are not on one line: {chip_tops:?}"
    );
}

fn chip_app() -> impl IntoElement {
    let chip = use_consume::<Chip>();
    rect().child(chip_element(&chip, Lane::new(0)))
}

/// The room is spent by a lower bound of each chip's width, so the column is always filled
/// to its edge: what a chip is counted at never exceeds what it is drawn at, for every kind
/// and for names of the narrowest characters. Caught by: a bound that ignores the cap or the
/// check mark, or a character advance wider than a narrow glyph's.
#[test]
fn the_room_a_chip_is_counted_by_never_exceeds_what_it_is_drawn_at() {
    let names = [
        "iiiiiiiiii",
        "l.l.l.l.l.",
        "main",
        "WWWW",
        "v1.0",
        "stash@{12}",
        "a/b/c-d_e",
    ];
    for kind in [
        ChipKind::Branch {
            current: false,
            tracked: false,
        },
        ChipKind::Branch {
            current: true,
            tracked: true,
        },
        ChipKind::Remote,
        ChipKind::Tag,
        ChipKind::Stash,
    ] {
        for name in names {
            let drawn = chip(kind, name);
            let counted = min_width(&drawn);
            let (mut test, ()) = TestingRunner::new(
                chip_app,
                (600., 100.).into(),
                {
                    let drawn = drawn.clone();
                    move |runner| {
                        runner.provide_root_context(move || drawn.clone());
                    }
                },
                1.,
            );
            test.sync_and_update();
            let width = test
                .find(|node, element| {
                    Rect::try_downcast(element)
                        .map(|_| node.layout().area)
                        .filter(|area| area.height() == CHIP_HEIGHT)
                        .map(|area| area.width())
                })
                .unwrap_or_else(|| panic!("no chip drawn"));
            let text = label_at(&test, name);
            assert!(
                text.1 > text.0,
                "no font drew {name:?}, so nothing was measured"
            );
            assert!(
                counted <= width,
                "{kind:?} {name:?} is counted at {counted} px but drawn at {width} px"
            );
        }
    }
}

/// The room a row's chips share is what its fixed columns and graph leave: the subject's
/// column, measured.
#[test]
fn the_room_is_the_subject_columns_width() {
    let test = draw(Vec::new(), false, WIDTH);
    let subject = label_at(&test, "the subject");
    let room = label_room(WIDTH, 1);
    assert!(
        (subject.1 - subject.0 - room).abs() < 0.5,
        "the subject is {} px wide and the room {room}",
        subject.1 - subject.0
    );
}
