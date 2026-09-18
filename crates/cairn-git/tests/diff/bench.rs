//! The reporter behind C14, and the measurement that answers Q3.
//!
//! Not an assertion: a timing check in CI is flaky and bound to a machine, which is why
//! `history-graph`'s A7 was not automated either. This prints numbers; the numbers go into
//! `docs/work/diff-engine/progress.md` beside git's own from
//! `docs/research/diff-engine/measured-baseline.md`.
//!
//! Run it with a release build, warm, against the repository the bar names:
//!
//! ```text
//! CAIRN_BENCH_REPO=~/Development/bench/rust \
//!   cargo test -p cairn-git --release --test diff_engine -- --ignored --nocapture
//! ```
//!
//! It only ever READS the repository it is pointed at.

use std::time::{Duration, Instant};

use cairn_git::{CancelSignal, ChangeSet, ChangesRequest, ContentOptions, DiffSession, Repository};
use cairn_model::{ChangeStatus, ChangedFile, DiffContent, Oid, SizeLimit};

/// The subjects `measured-baseline.md` chose, with git's own warm median beside each so a
/// run reads as a comparison rather than as a number on its own.
const CHANGES_SUBJECTS: &[(&str, &str, u128, f64)] = &[
    (
        "S7 f0845adb0c1 (1,017 edited files)",
        "f0845adb0c1b7a7fa1bef73e749b2d7e1d7f374d",
        100,
        7.9,
    ),
    (
        "S1 cf2dff2b1e3 (55,184 paths, 27,592 exact renames)",
        "cf2dff2b1e3fa55fa5415d524200070d0d7aacfe",
        500,
        28.6,
    ),
    (
        "M1 5a3292f163d (5,602 paths, 2,543 inexact renames)",
        "5a3292f163da3327523ddec5bc44d17c2378ec37",
        500,
        78.5,
    ),
];

/// git's own answer on M1, from `measured-baseline.md` section 2b: 2,774 renames, of which
/// 231 are exact and 2,543 are not.
const GIT_RENAMES_ON_M1: usize = 2_774;
const GIT_INEXACT_RENAMES_ON_M1: usize = 2_543;

const CONTENT_SUBJECT: (&str, &str, &str, u128, f64) = (
    "F7 3b09522c34b (about 10k changed lines)",
    "3b09522c34b43d8cc9334371ba7e54b8e06471d6",
    "library/stdarch/crates/core_arch/src/arm_shared/neon/generated.rs",
    100,
    9.8,
);

const TOO_LARGE_SUBJECT: (&str, &str, &str, f64) = (
    "F1 6a6e8446b97 (121k to 327k lines, 5.2 MB)",
    "6a6e8446b97e8a3dfc0984660253b1ac437a445a",
    "library/stdarch/intrinsics_data/arm_intrinsics.json",
    163.0,
);

const RUNS: usize = 5;

fn median(mut taken: Vec<Duration>) -> Duration {
    taken.sort_unstable();
    taken[taken.len() / 2]
}

fn report(name: &str, taken: Vec<Duration>, bar_ms: Option<u128>, git_ms: f64) {
    let best = taken.iter().min().copied().unwrap_or_default();
    let worst = taken.iter().max().copied().unwrap_or_default();
    let middle = median(taken);
    let bar = match bar_ms {
        Some(bar) => {
            let met = if middle.as_millis() <= bar {
                "MET"
            } else {
                "MISSED"
            };
            format!("\tbar {bar} ms {met}")
        }
        None => String::new(),
    };
    eprintln!(
        "  {name}\n    median {:.3} ms\tmin {:.3}\tmax {:.3}\tgit {git_ms:.1} ms{bar}",
        middle.as_secs_f64() * 1000.0,
        best.as_secs_f64() * 1000.0,
        worst.as_secs_f64() * 1000.0,
    );
}

fn pairs(set: &ChangeSet) -> (usize, usize) {
    let renames = set.files.iter().filter(|file| file.is_rename()).count();
    let exact = set
        .files
        .iter()
        .filter(|file| match file.status {
            ChangeStatus::Renamed(similarity) => similarity.percent() == 100,
            ChangeStatus::Added
            | ChangeStatus::Deleted
            | ChangeStatus::Modified
            | ChangeStatus::TypeChanged
            | ChangeStatus::Copied(_) => false,
        })
        .count();
    (renames, exact)
}

fn file_at<'a>(set: &'a ChangeSet, path: &str) -> &'a ChangedFile {
    set.files
        .iter()
        .find(|file| file.new_path.display() == path)
        .unwrap_or_else(|| panic!("{path} did not change in this commit"))
}

fn changes(session: &mut DiffSession<'_>, id: &Oid) -> ChangeSet {
    super::ok(
        session.changes(&ChangesRequest::commit(*id), &CancelSignal::new()),
        "the changes query answers",
    )
}

#[test]
#[ignore = "needs a large repository named by CAIRN_BENCH_REPO"]
fn measures_the_diff_queries_against_a_named_repository() {
    let path = std::env::var("CAIRN_BENCH_REPO").expect("set CAIRN_BENCH_REPO");
    let engine = Repository::discover(&path).expect("the bench repository opens");
    eprintln!(
        "repository {path}\n  release build: {}\n",
        !cfg!(debug_assertions)
    );

    eprintln!("changes query, a session per query (what a cold worker pays):");
    for (name, hex, bar, git_ms) in CHANGES_SUBJECTS {
        let id = Oid::parse(hex).expect("an id");
        let mut taken = Vec::new();
        let mut files = 0usize;
        for _ in 0..RUNS {
            let started = Instant::now();
            let mut session = engine.diff_session().expect("a diff session");
            let set = changes(&mut session, &id);
            taken.push(started.elapsed());
            files = set.files.len();
        }
        report(name, taken, Some(*bar), *git_ms);
        eprintln!("    {files} changed files");
    }

    eprintln!("\nchanges query, on a session already open (what phase 04's worker pays):");
    let mut session = engine.diff_session().expect("a diff session");
    for (name, hex, bar, git_ms) in CHANGES_SUBJECTS {
        let id = Oid::parse(hex).expect("an id");
        let mut taken = Vec::new();
        for _ in 0..RUNS {
            let started = Instant::now();
            let _ = changes(&mut session, &id);
            taken.push(started.elapsed());
        }
        report(name, taken, Some(*bar), *git_ms);
    }

    // Q3: how far gix's rename detection is from git's on the largest rollup.
    let m1 = Oid::parse(CHANGES_SUBJECTS[2].1).expect("an id");
    let set = changes(&mut session, &m1);
    let (renames, exact) = pairs(&set);
    eprintln!(
        "\nQ3, rename pairs on 5a3292f163d:\n  \
         gix {renames} renames ({exact} of them at 100%), git {GIT_RENAMES_ON_M1} \
         ({} inexact)\n  gap {}",
        GIT_INEXACT_RENAMES_ON_M1,
        renames as i64 - GIT_RENAMES_ON_M1 as i64
    );
    eprintln!(
        "  detection: {:?}\n  cut short: {}",
        set.renames,
        set.renames.was_cut_short()
    );

    eprintln!("\ncontent query:");
    let (name, hex, path, bar, git_ms) = CONTENT_SUBJECT;
    let id = Oid::parse(hex).expect("an id");
    let set = changes(&mut session, &id);
    let file = file_at(&set, path).clone();
    // 2.5 MB, so the default query refuses it on R2.6's byte ceiling; what the bar is about
    // is the time to ANSWER it, which is the time to load it.
    let anyway = ContentOptions {
        load_anyway: true,
        ..ContentOptions::default()
    };
    let mut refused = Vec::new();
    let mut taken = Vec::new();
    for _ in 0..RUNS {
        let started = Instant::now();
        let refusal = session
            .file_diff(&file, &ContentOptions::default())
            .expect("a file diff");
        refused.push(started.elapsed());
        assert!(
            matches!(refusal.content, DiffContent::TooLarge { .. }),
            "{path} is 2.5 MB and should be refused by default: {:?}",
            refusal.content
        );

        let started = Instant::now();
        let diff = session.file_diff(&file, &anyway).expect("a file diff");
        taken.push(started.elapsed());
        assert!(
            matches!(diff.content, DiffContent::Text { .. }),
            "{path} is not text: {:?}",
            diff.content
        );
    }
    report(
        &format!("{name}, refused on the byte ceiling"),
        refused,
        None,
        git_ms,
    );
    report(&format!("{name}, loaded"), taken, Some(bar), git_ms);

    eprintln!("\nthe file that is too large to draw, and what loading it anyway costs:");
    let (name, hex, path, git_ms) = TOO_LARGE_SUBJECT;
    let id = Oid::parse(hex).expect("an id");
    let set = changes(&mut session, &id);
    let file = file_at(&set, path).clone();
    let mut refusing = Vec::new();
    for _ in 0..RUNS {
        let started = Instant::now();
        let diff = session
            .file_diff(&file, &ContentOptions::default())
            .expect("a file diff");
        refusing.push(started.elapsed());
        let DiffContent::TooLarge { crossed, loadable } = diff.content else {
            panic!("{path} was not refused: {:?}", diff.content);
        };
        assert!(
            matches!(crossed, SizeLimit::Bytes { .. }),
            "{path} crossed {crossed:?}, not the byte ceiling"
        );
        assert!(
            loadable,
            "{path} is under 64 MiB, so it can be loaded anyway"
        );
    }
    report(&format!("{name}, refused"), refusing, None, git_ms);

    let anyway = ContentOptions {
        load_anyway: true,
        ..ContentOptions::default()
    };
    let mut loading = Vec::new();
    for _ in 0..RUNS {
        let started = Instant::now();
        let diff = session.file_diff(&file, &anyway).expect("a file diff");
        loading.push(started.elapsed());
        assert!(matches!(diff.content, DiffContent::Text { .. }));
    }
    report(&format!("{name}, Load Diff"), loading, None, git_ms);
}
