//! C16 on real repositories: every row a history pages out draws the subject, author, date,
//! short id and merge marker it drew before rows were slimmed.
//!
//! The tables below are what the rows drew before: each was asserted against the old rows'
//! own fields (their full parents, their author's address beside the name) while those
//! fields still rode on every row — commit `4205d5d`, the one before the rows were slimmed
//! — and the tests now hold the slimmed rows to them. The
//! crafted fixture is built by `git commit-tree`, so its ids are the same on every machine;
//! the Cairn checkout's table is the walk from `main` as it stood when the rows were
//! slimmed, which is never rewritten. Every row of both, and of the Cairn checkout from
//! `HEAD`, is also checked against the details query, which reads the commit afresh.

mod fixtures;

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use cairn_git::{CancelSignal, HistoryRequest, Repository};
use cairn_model::{History, Oid, RowContent};

use fixtures::Fixture;

fn ok<T, E: std::fmt::Display>(result: Result<T, E>, what: &str) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("{what}: {error}"),
    }
}

/// What a row draws: its id (the list shows its short form), subject, author, author date
/// and how many parents it has (a merge's node is a ring).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Drawn {
    id: String,
    subject: String,
    author: String,
    time: i64,
    parents: usize,
}

/// A row as a table writes it; `subject` [`LONG`] stands for [`long_subject`].
type Row = (&'static str, &'static str, &'static str, i64, usize);

const LONG: &str = "<long>";

/// Longer than a whole chunk of any text store, so it can never share one.
fn long_subject() -> String {
    (0..7_000)
        .map(|n| format!("{:09} ", n))
        .collect::<String>()
        .trim_end()
        .to_owned()
}

fn expected(table: &[Row]) -> Vec<Drawn> {
    table
        .iter()
        .map(|&(id, subject, author, time, parents)| Drawn {
            id: id.to_owned(),
            subject: if subject == LONG {
                long_subject()
            } else {
                subject.to_owned()
            },
            author: author.to_owned(),
            time,
            parents,
        })
        .collect()
}

/// What each row draws, read through the history that holds it. No wildcard arm.
fn drawn_rows(rows: &History) -> Vec<Drawn> {
    rows.rows()
        .map(|row| match row.content() {
            RowContent::Commit(commit) => Drawn {
                id: commit.id.to_string(),
                subject: commit.summary,
                author: commit.author_name,
                time: commit.author_time,
                parents: commit.parent_count,
            },
        })
        .collect()
}

/// Pages a held session to the end, `page` rows at a time, appending each page to one
/// history as the window does.
fn page_all(repo: &Repository, request: &HistoryRequest, page: usize) -> History {
    let mut session = ok(repo.history_session(request), "opening the session");
    let mut rows = History::new();
    loop {
        let next = ok(session.next_page(page, &CancelSignal::new()), "paging");
        ok(rows.append(next.rows), "holding a page");
        if next.cursor.is_none() {
            return rows;
        }
    }
}

/// The same rows as the details query reads each commit afresh: its id, author, date and
/// parents. Not its subject: the Commit tab's is the message's first line, where a row's
/// folds the first paragraph as git's `%s` does (`CommitDetails::subject`).
fn as_details_read_them(repo: &Repository, drawn: &[Drawn]) -> Vec<Drawn> {
    drawn
        .iter()
        .map(|row| {
            let id = ok(Oid::parse(&row.id), "a row's id");
            let details = ok(repo.commit_details(&id), "reading a commit's details");
            Drawn {
                id: details.id.to_string(),
                subject: row.subject.clone(),
                author: details.author.name.clone(),
                time: details.author.time.seconds,
                parents: details.parents.len(),
            }
        })
        .collect()
}

/// The same rows as `git log` prints each commit: `%s`, `%an`, `%at` and `%P`. Only for a
/// history whose every message and name is UTF-8, which git prints as it is stored.
fn as_git_prints_them(dir: &Path, drawn: &[Drawn]) -> Vec<Drawn> {
    drawn
        .iter()
        .map(|row| {
            let printed = fixtures::run(
                dir,
                &[
                    "log",
                    "-1",
                    "--format=%H%x1f%s%x1f%an%x1f%at%x1f%P",
                    &row.id,
                ],
                None,
            );
            let fields: Vec<&str> = printed.trim_end_matches('\n').split('\u{1f}').collect();
            let [id, subject, author, time, parents] = fields.as_slice() else {
                panic!("git log printed {printed:?} for {}", row.id);
            };
            Drawn {
                id: (*id).to_owned(),
                subject: (*subject).to_owned(),
                author: (*author).to_owned(),
                time: ok(time.parse(), "an author date"),
                parents: parents.split_whitespace().count(),
            }
        })
        .collect()
}

/// Row by row, so a failure names the row rather than printing two whole histories.
fn assert_draws(what: &str, drawn: &[Drawn], before: &[Drawn]) {
    for (index, (now, then)) in drawn.iter().zip(before).enumerate() {
        assert_eq!(now, then, "{what}: row {index} draws something else");
    }
    assert_eq!(
        drawn.len(),
        before.len(),
        "{what}: a row was lost or gained"
    );
}

// --- The crafted fixture ---

/// A repository built commit by commit with `git commit-tree`, each commit's author, date,
/// parents and message bytes chosen, so every id is fixed.
struct Crafted {
    fixture: Fixture,
    home: PathBuf,
}

impl Crafted {
    fn new() -> Self {
        let fixture = fixtures::unborn();
        let home = fixture.path().with_extension("home");
        ok(std::fs::create_dir_all(&home), "making an empty home");
        Self { fixture, home }
    }

    /// Commits `message`'s bytes on `parents` by `author` at `at`, named in `encoding` when
    /// given; returns its id.
    fn commit(
        &self,
        parents: &[&str],
        message: &[u8],
        author: &str,
        at: i64,
        encoding: Option<&str>,
    ) -> String {
        let empty_tree = self
            .fixture
            .git(&["hash-object", "-t", "tree", "--stdin"])
            .trim()
            .to_owned();
        let mut args: Vec<String> = Vec::new();
        if let Some(encoding) = encoding {
            args.extend(["-c".to_owned(), format!("i18n.commitEncoding={encoding}")]);
        }
        args.extend(["commit-tree".to_owned(), empty_tree]);
        for parent in parents {
            args.extend(["-p".to_owned(), (*parent).to_owned()]);
        }
        let stamp = format!("{at} +0000");
        let mut child = ok(
            Command::new("git")
                .current_dir(self.fixture.path())
                .args(&args)
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_CONFIG_SYSTEM", "/dev/null")
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("HOME", &self.home)
                .env("XDG_CONFIG_HOME", &self.home)
                .env("GIT_AUTHOR_NAME", author)
                .env("GIT_AUTHOR_EMAIL", "author@example.com")
                .env("GIT_COMMITTER_NAME", "C O Mitter")
                .env("GIT_COMMITTER_EMAIL", "committer@example.com")
                .env("GIT_AUTHOR_DATE", &stamp)
                .env("GIT_COMMITTER_DATE", &stamp)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn(),
            "starting git commit-tree",
        );
        if let Some(mut stdin) = child.stdin.take() {
            ok(stdin.write_all(message), "writing the message");
        }
        let output = ok(child.wait_with_output(), "running git commit-tree");
        assert!(
            output.status.success(),
            "git commit-tree failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }

    fn path(&self) -> &Path {
        self.fixture.path()
    }
}

impl Drop for Crafted {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.home);
    }
}

/// Ten commits, paged two at a time from `tip`, newest first:
///
/// ```text
/// tip   Ada       "Tip"                                  one parent
/// oct   Margaret  a two-line first paragraph, folded      three parents (an octopus)
/// merge Ada       "Merge side work"                      two parents
/// bad   Grace     bytes that are not UTF-8, no encoding  one parent
/// third Margaret  "Third line"                           one parent
/// side  Zoë       "Side work", a name past ASCII         one parent
/// latin Grace     "Café crème" in ISO-8859-1, so named   one parent
/// long  Ada       a subject longer than any text chunk   one parent
/// empty Grace     no message at all                      one parent
/// root  Ada       "Begin the walk"                       none
/// ```
///
/// so the first page brings two authors, the second one new and one known, the third one
/// new and one known, and the fourth and fifth only known ones.
fn crafted() -> (Crafted, String) {
    let at = |seconds: i64| fixtures::EPOCH + seconds;
    let repo = Crafted::new();
    let root = repo.commit(&[], b"Begin the walk\n", "Ada Lovelace", at(60), None);
    let empty = repo.commit(&[&root], b"", "Grace Hopper", at(120), None);
    let long = repo.commit(
        &[&empty],
        format!("{}\n\nAnd a body.\n", long_subject()).as_bytes(),
        "Ada Lovelace",
        at(180),
        None,
    );
    let latin = repo.commit(
        &[&long],
        b"Caf\xe9 cr\xe8me\n",
        "Grace Hopper",
        at(240),
        Some("ISO-8859-1"),
    );
    let side = repo.commit(
        &[&long],
        b"Side work\n",
        "Zo\u{eb} \u{c5}ngstr\u{f6}m",
        at(250),
        None,
    );
    let third = repo.commit(
        &[&latin],
        b"Third line\n",
        "Margaret Hamilton",
        at(270),
        None,
    );
    let bad = repo.commit(
        &[&latin],
        b"Bad \xff\xfe bytes\n",
        "Grace Hopper",
        at(300),
        None,
    );
    let merge = repo.commit(
        &[&bad, &side],
        b"Merge side work\n",
        "Ada Lovelace",
        at(360),
        None,
    );
    let octopus = repo.commit(
        &[&merge, &third, &root],
        b"first line\nsecond line\n\nThe body.\n",
        "Margaret Hamilton",
        at(420),
        None,
    );
    let tip = repo.commit(&[&octopus], b"Tip\n", "Ada Lovelace", at(480), None);
    (repo, tip)
}

/// What the crafted fixture's rows drew before rows were slimmed.
const CRAFTED_BEFORE: &[Row] = &[
    (
        "d64374098f705d852a54acdbf504e5354bb69fdb",
        "Tip",
        "Ada Lovelace",
        1500000480,
        1,
    ),
    (
        "65cb204ea828be9586d21f6e9df93febfddde9d7",
        "first line second line",
        "Margaret Hamilton",
        1500000420,
        3,
    ),
    (
        "a383cbed6368e404a35362739f056e9050bfd68f",
        "Merge side work",
        "Ada Lovelace",
        1500000360,
        2,
    ),
    (
        "15662a81ed76c4dc52f7f681f4e40826c09b9484",
        "Bad ÿþ bytes",
        "Grace Hopper",
        1500000300,
        1,
    ),
    (
        "0fa0b5bfbfa2b3ae336fab79ebae4aa30a797e9b",
        "Third line",
        "Margaret Hamilton",
        1500000270,
        1,
    ),
    (
        "f3d95223ad4018f3b1c91d65844286f627a144d0",
        "Side work",
        "Zoë Ångström",
        1500000250,
        1,
    ),
    (
        "a2011764d69fc6f85ccb089130e9a0b292f7d88c",
        "Café crème",
        "Grace Hopper",
        1500000240,
        1,
    ),
    (
        "001869909e1b7033cfec7a051146727f8740fa42",
        LONG,
        "Ada Lovelace",
        1500000180,
        1,
    ),
    (
        "cbb63003f7fbfac99677adb5a0ef5a4c2e86990a",
        "",
        "Grace Hopper",
        1500000120,
        1,
    ),
    (
        "b874dc9552507aabe3f1d16eb2195a62650f556d",
        "Begin the walk",
        "Ada Lovelace",
        1500000060,
        0,
    ),
];

#[test]
fn every_crafted_row_draws_what_it_drew_before_rows_were_slimmed() {
    let (fixture, tip) = crafted();
    let repo = ok(Repository::discover(fixture.path()), "opening the fixture");
    let request = HistoryRequest::from_commits([ok(Oid::parse(&tip), "the tip")], 2);
    let rows = page_all(&repo, &request, 2);
    let drawn = drawn_rows(&rows);
    assert_draws("the crafted fixture", &drawn, &expected(CRAFTED_BEFORE));
    // Five pages: two new authors, then a new one beside a known one twice, then only
    // known ones twice. Each is named once.
    assert_eq!(
        rows.author_count(),
        4,
        "an author was named twice, or one was lost"
    );
    assert_draws(
        "the crafted fixture, read afresh",
        &drawn,
        &as_details_read_them(&repo, &drawn),
    );
}

// --- The Cairn checkout ---

/// `main` when rows were slimmed: the squash-merged planning of this packet.
const MAIN_WHEN_SLIMMED: &str = "0cfd746";

/// What the Cairn checkout's rows from [`MAIN_WHEN_SLIMMED`] drew before rows were slimmed.
const CAIRN_BEFORE: &[Row] = &[
    (
        "0cfd746b5a913529bbd1a9684f8e92f1cb364c84",
        "docs(prd): plan the refs-and-status packet, from evidence and a Fork study (#60)",
        "Alexander Parlett",
        1791263589,
        1,
    ),
    (
        "0a7aeb78b918a341bc21109059a240bc64924644",
        "fix(app): let the worker tests search a short checkout history (#61)",
        "Alexander Parlett",
        1791229015,
        1,
    ),
    (
        "9b6b7d0faf648d82c3da02297afc3777e969b827",
        "diff-engine: show what a commit changed as git answers it, drawn as Fork draws it (#59)",
        "Alexander Parlett",
        1791207676,
        1,
    ),
    (
        "cfe33151ce42afd96a6edfe5e8ee22a6489cde48",
        "process-manager: one place that starts git, and a runner every verb can share (#50)",
        "Alexander Parlett",
        1791005826,
        1,
    ),
    (
        "4fe7165b6f155b883e3fd960b63d27a91f573a02",
        "docs(prd): plan the process-manager packet, from evidence (#40)",
        "Alexander Parlett",
        1790960527,
        1,
    ),
    (
        "2987a53d1408bf625c6ae730f81ec4a312040614",
        "docs(docs): keep point-in-time packet state out of design/ (#39)",
        "Alexander Parlett",
        1790142324,
        1,
    ),
    (
        "7d4d9ad7a2e366ca3a3d6277079d493d44c4560f",
        "docs(prd): plan the diff-engine packet, from evidence and a Fork study (#38)",
        "Alexander Parlett",
        1789671788,
        1,
    ),
    (
        "bf93a4eed43422a8f8f7cfbcda29c852d694ad44",
        "credential-prompts: authenticated fetch without Cairn holding a credential (#28)",
        "Alexander Parlett",
        1789653427,
        1,
    ),
    (
        "f4f1d0512492d07fa38eb36eb8105e884d79090d",
        "history-graph: draw the commit graph of a real repository (#14)",
        "Alexander Parlett",
        1789592299,
        1,
    ),
    (
        "9c93114bfba6a280b4965c42ac56a9079396e7f3",
        "Merge pull request #15 from alexparlett/docs/ui-design",
        "Alexander Parlett",
        1789584164,
        2,
    ),
    (
        "e480fddafc07897e52b1cd9b9a69dd1c9f9fddb1",
        "docs(design): UI design modelled on Fork, with five annotated mockups",
        "Alex Parlett",
        1789584018,
        1,
    ),
    (
        "7f9c64852879d27ff41ecc2b247141fc8b6258b9",
        "docs(design): stamp the out-of-scope list as reviewed against real usage",
        "Alex Parlett",
        1789417878,
        1,
    ),
    (
        "b8f51524295a5709d685b0ee4a1dd217b241d457",
        "docs(design): forge links are in scope (D9); the old line conflated two things",
        "Alex Parlett",
        1789417616,
        1,
    ),
    (
        "1391fb127b2d84dc9aa9f0125e3ca63d066083b7",
        "docs(design): inventory the feature surface, plan the daily-loop program",
        "Alex Parlett",
        1789417276,
        1,
    ),
    (
        "3c1f0b34a7f47d68a7e689331eb3de1f8d4b4cc2",
        "docs(design): order the packets, and close two gaps in the graph plan",
        "Alex Parlett",
        1789416833,
        1,
    ),
    (
        "43602a3042ed2c6ec38a6039eab5f927cf0d0cc9",
        "docs(design): close four open questions, fix two I framed wrongly",
        "Alex Parlett",
        1789415060,
        1,
    ),
    (
        "d23b4ff446097e9930ca30c16364b279b906bd0e",
        "docs(design): lock decisions D1-D5 and file two packets",
        "Alex Parlett",
        1789410340,
        1,
    ),
    (
        "382d6980fe973f4b6e92fbd94651bead5b4f88cd",
        "chore(repo): adopt the agentic harness and scaffold the Cairn workspace",
        "Alex Parlett",
        1789409202,
        0,
    ),
];

fn cairn() -> Repository {
    ok(
        Repository::discover(env!("CARGO_MANIFEST_DIR")),
        "opening the Cairn checkout",
    )
}

#[test]
fn every_row_of_the_cairn_checkout_draws_what_it_drew_before_rows_were_slimmed() {
    let repo = cairn();
    let main = fixtures::run(
        Path::new(env!("CARGO_MANIFEST_DIR")),
        &[
            "rev-parse",
            "--verify",
            &format!("{MAIN_WHEN_SLIMMED}^{{commit}}"),
        ],
        None,
    );
    let main = ok(Oid::parse(main.trim()), "main's id");
    let rows = page_all(&repo, &HistoryRequest::from_commits([main], 5), 5);
    let drawn = drawn_rows(&rows);
    assert_draws("the Cairn checkout's main", &drawn, &expected(CAIRN_BEFORE));

    let everything = drawn_rows(&page_all(&repo, &HistoryRequest::from_head(64), 64));
    assert!(
        everything.len() > drawn.len(),
        "the walk from HEAD holds no more than main's"
    );
    assert_draws(
        "the Cairn checkout from HEAD, read afresh",
        &everything,
        &as_details_read_them(&repo, &everything),
    );
    assert_draws(
        "the Cairn checkout from HEAD, as git prints it",
        &everything,
        &as_git_prints_them(Path::new(env!("CARGO_MANIFEST_DIR")), &everything),
    );
}

/// A braided history of merges, paged as the window pages it: every row draws what `git log`
/// prints for it, in `git rev-list`'s order.
#[test]
fn every_row_of_a_braided_history_draws_what_git_prints() {
    let fixture = fixtures::braided(40);
    let repo = ok(Repository::discover(fixture.path()), "opening the fixture");
    let drawn = drawn_rows(&page_all(&repo, &HistoryRequest::from_head(7), 7));
    let ids: Vec<String> = drawn.iter().map(|row| row.id.clone()).collect();
    assert_eq!(ids, fixture.rev_list(), "the walk is not git's");
    assert!(
        drawn.iter().any(|row| row.parents > 1),
        "no merge, so no merge marker was decided"
    );
    assert_draws(
        "a braided history",
        &drawn,
        &as_git_prints_them(fixture.path(), &drawn),
    );
}
