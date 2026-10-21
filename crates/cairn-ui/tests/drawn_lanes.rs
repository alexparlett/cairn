//! Where a stash's row is drawn — on its base's lane, as Fork for Windows draws it — read
//! back from the painted list; and what drawing a viewport of history rows costs when
//! thousands of lines are open across it — the widest the graph gets, every unmerged ref one
//! line — and what laying out a row's chips costs when its commit carries thousands of refs.
//! The costs are a measurement, `#[ignore]`d: timings in CI are flaky and bound to a machine.
//! Run in release:
//! `cargo test --release -p cairn-ui --test drawn_lanes -- --ignored --nocapture`.

use std::time::{Duration, Instant};

use cairn_model::{
    GraphRow, History, Label, Lane, LaneAssigner, Oid, PagedCommit, PagedStash, RefKind,
    RowContent, RowsPage,
};
use cairn_ui::graph_geometry::{lane_x, row_middle};
use cairn_ui::{CommitRow, HistoryList, ROW_HEIGHT, ROW_PADDING, RowRender, row_chips};
use freya::engine::prelude::{Image, ImageInfo, raster_n32_premul};
use freya::prelude::*;
use freya_testing::TestingRunner;

const WIDTH: f32 = 1_400.;
const HEIGHT: f32 = 900.;
const MAIN: usize = 12_000;
const RUNS: usize = 7;

fn oid(n: usize) -> Oid {
    let mut bytes = [0u8; 20];
    bytes[0] = 0x10 + (n % 200) as u8;
    bytes[12..20].copy_from_slice(&(n as u64).to_be_bytes());
    Oid::from_bytes(&bytes).unwrap_or_else(|_| unreachable!("20 bytes is a SHA-1"))
}

/// The fixture `measures_layout_over_unmerged_refs` walks (`crates/cairn-git/tests/
/// every_ref.rs`), laid out in its walk order: `branches` one-commit branches newer than
/// everything, forked at distinct commits down the upper half of a `MAIN`-commit line, so
/// every branch's line stays open until its fork — each branch tip labelled by its branch.
fn wide(branches: usize) -> History {
    let main = |n: usize| oid(1_000_000 + n);
    let mut walk: Vec<(Oid, Vec<Oid>, Option<String>)> = Vec::new();
    for b in 0..branches {
        let fork = MAIN - 1 - (b * (MAIN / 2)) / branches.max(1);
        walk.push((
            oid(b),
            vec![main(fork)],
            Some(format!("refs/heads/b{b:05}")),
        ));
    }
    for n in (0..MAIN).rev() {
        let parents = if n == 0 {
            Vec::new()
        } else {
            vec![main(n - 1)]
        };
        walk.push((main(n), parents, None));
    }
    let rows =
        LaneAssigner::new().assign_each(walk.iter().map(|(id, parents, _)| (*id, parents.clone())));
    let mut history = History::new();
    let mut page = RowsPage::new();
    for (row, (_, parents, label)) in rows.into_iter().zip(&walk) {
        let commit = PagedCommit {
            parents: parents.len(),
            subject: "a commit on a wide graph",
            author: "Ada",
            author_time: 0,
        };
        match label {
            Some(name) => page.push_labelled(
                row,
                commit,
                false,
                &[Label {
                    name,
                    kind: RefKind::LocalBranch,
                    current: false,
                }],
            ),
            None => page.push(row, commit),
        }
        if page.len() == 4_096 {
            history
                .append(std::mem::take(&mut page))
                .unwrap_or_else(|full| panic!("{full}"));
        }
    }
    history.append(page).unwrap_or_else(|full| panic!("{full}"));
    history
}

fn app() -> Element {
    let rows = use_consume::<State<History>>();
    let lanes = use_consume::<usize>();
    HistoryList::new(rows, |render: RowRender| {
        let commit = match render.content {
            RowContent::Commit(commit) => commit,
            RowContent::Stash(stash) => stash.as_commit(),
        };
        CommitRow::new(commit, render.graph, render.lanes)
            .chips(render.chips)
            .head(render.head)
            .selected(render.selected)
            .into()
    })
    .lanes(lanes)
    .into()
}

fn median(mut samples: Vec<Duration>) -> f64 {
    samples.sort_unstable();
    samples[samples.len() / 2].as_secs_f64() * 1e3
}

#[test]
#[ignore = "a measurement; run with --release"]
fn measures_drawing_a_viewport_over_many_open_lanes_and_a_crowded_rows_chips() {
    eprintln!("| Open lines | Where | Build and lay out | Paint |");
    eprintln!("| --- | --- | --- | --- |");
    for branches in [100usize, 1_000, 5_000] {
        let history = wide(branches);
        let lanes = history
            .rows()
            .map(|row| row.lanes_named())
            .max()
            .unwrap_or(1);
        let (mut test, ()) = TestingRunner::new(
            app,
            (WIDTH, HEIGHT).into(),
            move |runner| {
                runner.provide_root_context(move || State::create(history));
                runner.provide_root_context(move || lanes);
            },
            1.,
        );
        test.sync_and_update();
        test.sync_and_update();
        // Under every line: the last branch tip's row, where all of them are open; then the
        // middle of the main line, where half have joined.
        for (place, row) in [
            ("under every line", branches),
            ("mid-line", branches + MAIN / 2),
        ] {
            test.scroll((100., 100.), (0., 1e9));
            test.scroll((100., 100.), (0., -(row as f64) * f64::from(ROW_HEIGHT)));
            let (mut built, mut painted) = (Vec::new(), Vec::new());
            for run in 0..=RUNS {
                // One row down and back: every row of the viewport is built again.
                let step = if run % 2 == 0 { -1. } else { 1. };
                let started = Instant::now();
                test.scroll((100., 100.), (0., step * f64::from(ROW_HEIGHT)));
                let laid = started.elapsed();
                let started = Instant::now();
                let _ = test.render();
                let paint = started.elapsed();
                if run > 0 {
                    built.push(laid);
                    painted.push(paint);
                }
            }
            eprintln!(
                "| {branches} | {place} | {:.2} ms | {:.2} ms |",
                median(built),
                median(painted)
            );
        }
    }

    // One commit carrying thousands of refs: the chips laid out for a 600 px column.
    let names: Vec<String> = (0..10_000).map(|n| format!("refs/tags/t{n:05}")).collect();
    let labels: Vec<Label<'_>> = names
        .iter()
        .map(|name| Label {
            name,
            kind: RefKind::Tag,
            current: false,
        })
        .collect();
    let mut page = RowsPage::new();
    page.push_labelled(
        GraphRow::new(oid(1), cairn_model::Lane::new(0), Vec::new()),
        PagedCommit {
            parents: 0,
            subject: "crowded",
            author: "Ada",
            author_time: 0,
        },
        false,
        &labels,
    );
    let mut history = History::new();
    history.append(page).unwrap_or_else(|full| panic!("{full}"));
    let row = history.row(0).unwrap_or_else(|| panic!("no row"));
    let mut samples = Vec::new();
    let mut built = 0;
    for _ in 0..101 {
        let started = Instant::now();
        built = row_chips(row.labels(), None, 600.).len();
        samples.push(started.elapsed());
    }
    eprintln!(
        "\nA row of 10,000 refs: {built} chips laid out for 600 px in {:.4} ms (median of 101)",
        median(samples)
    );
}

/// One step of a walk: a commit with its parents, or a stash on its base.
enum Step {
    Commit(Oid, Vec<Oid>),
    Stash(Oid, Oid),
}

/// `walk` laid out as the engine lays it out, and held as one history.
fn laid_out(walk: &[Step]) -> History {
    let mut assigner = LaneAssigner::new();
    let mut graphs = Vec::new();
    for step in walk {
        let row = match step {
            Step::Commit(id, parents) => assigner.push(*id, parents.clone()),
            Step::Stash(id, base) => assigner.push_stash(*id, *base),
        };
        graphs.extend(row);
    }
    graphs.extend(assigner.into_rows());
    let mut page = RowsPage::new();
    for (graph, step) in graphs.into_iter().zip(walk) {
        match step {
            Step::Commit(_, parents) => page.push(
                graph,
                PagedCommit {
                    parents: parents.len(),
                    subject: "a commit",
                    author: "Ada",
                    author_time: 0,
                },
            ),
            Step::Stash(_, base) => page.push_stash(
                graph,
                PagedStash {
                    index: 0,
                    base: *base,
                    message: "On main: WIP",
                    author: "Ada",
                    author_time: 0,
                },
            ),
        }
    }
    let mut history = History::new();
    history.append(page).unwrap_or_else(|full| panic!("{full}"));
    history
}

/// Whether the pixel at the node of lane `lane` on row `row` of the drawn list is painted —
/// anything but the history's ground.
fn painted_at_node(
    test: &mut TestingRunner,
    width: f32,
    height: f32,
    row: usize,
    lane: usize,
) -> bool {
    let png = test.render();
    let image = Image::from_encoded(png).unwrap_or_else(|| panic!("the frame did not decode"));
    let (w, h) = (width as i32, height as i32);
    let mut surface = raster_n32_premul((w, h)).unwrap_or_else(|| panic!("no raster surface"));
    surface.canvas().draw_image(&image, (0, 0), None);
    let info = ImageInfo::new_n32_premul((w, h), None);
    let stride = info.min_row_bytes();
    let mut pixels = vec![0u8; stride * h as usize];
    assert!(surface.read_pixels(&info, &mut pixels, stride, (0, 0)));
    let ground = cairn_ui::style::tones::HISTORY;
    let x = ROW_PADDING + lane_x(Lane::new(lane));
    let y = row as f32 * ROW_HEIGHT + row_middle();
    let at = y.round() as usize * stride + x.round() as usize * 4;
    pixels
        .get(at..at + 3)
        .is_some_and(|bgr| [bgr[2], bgr[1], bgr[0]] != [ground.r(), ground.g(), ground.b()])
}

/// A stash whose base a line already awaits sits on that line — its node in the base's lane,
/// no lane of its own beside it — and a stash whose base no line awaits takes the next free
/// lane, which its base then takes; drawn by the window's own row, read back from the paint.
/// Caught by: a stash given a lane of its own beside its base's line (its node painted one
/// lane to the right), or one put on a line awaiting another commit.
#[test]
fn a_stash_is_drawn_on_its_base_commits_lane() {
    let id = |n: u8| {
        let mut bytes = [0x5au8; 20];
        bytes[0] = n;
        Oid::from_bytes(&bytes).unwrap_or_else(|_| unreachable!("20 bytes is a SHA-1"))
    };
    let (other, child, stash, base, root) = (id(1), id(2), id(3), id(4), id(5));
    // `child` waits for `base` in lane 1, beside `other`'s line in lane 0.
    let awaited = laid_out(&[
        Step::Commit(other, vec![root]),
        Step::Commit(child, vec![base]),
        Step::Stash(stash, base),
        Step::Commit(base, vec![root]),
        Step::Commit(root, Vec::new()),
    ]);
    // Nothing waits for `base`: the stash takes lane 1, beside `other`'s line.
    let alone = laid_out(&[
        Step::Commit(other, vec![root]),
        Step::Stash(stash, base),
        Step::Commit(base, vec![root]),
        Step::Commit(root, Vec::new()),
    ]);
    for (history, stash_row, base_row) in [(awaited, 2, 3), (alone, 1, 2)] {
        let lanes: Vec<usize> = history.rows().map(|row| row.lane().index()).collect();
        assert_eq!(
            lanes.get(stash_row),
            lanes.get(base_row),
            "the stash on its base's lane: {lanes:?}"
        );
        assert_eq!(lanes.get(stash_row), Some(&1), "{lanes:?}");
        let (width, height) = (700., 200.);
        let (mut test, ()) = TestingRunner::new(
            app,
            (width, height).into(),
            move |runner| {
                runner.provide_root_context(move || State::create(history));
                runner.provide_root_context(|| 3usize);
            },
            1.,
        );
        test.sync_and_update();
        test.sync_and_update();
        assert!(
            painted_at_node(&mut test, width, height, stash_row, 1),
            "the stash's node is not drawn in its base's lane"
        );
        assert!(
            !painted_at_node(&mut test, width, height, stash_row, 2),
            "the stash drew a lane of its own"
        );
    }
}
