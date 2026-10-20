//! The toolbar's status box, Fork's (`docs/research/ui-review/fork-window-survey-2026-10-16.md`,
//! section 1): two centred lines, the repository's name bold — its folder's, as the worker that
//! opened it names it — over the branch glyph and the current branch. A detached `HEAD` names
//! its short id, an unborn branch its name and that it has no commit yet, each in git's own
//! words; a branch whose upstream is gone says so after it, git's `[gone]`. How far the branch is
//! from its upstream is not said here: the toolbar draws it as badges on Push and Pull.
//!
//! It is also where progress shows, as Fork's does while an operation runs
//! (`docs/research/staging-and-commit/fork-activity-result-lines-2026-10-11.md`, ar22z): a
//! spinner and the running operation's one name — a local write, " · N waiting" behind it while
//! writes queue, or a fetch — over git's latest progress line up to its percentage
//! ([`progress_text`]; the whole line is the activity popover's), in place of the repository's
//! lines, each operation only once it has run [`PROGRESS_AFTER`], so a write that is over at once
//! draws nothing. The wait is a task on the toolkit's executor (`async_io::Timer`), never a
//! sleep.

use std::time::{Duration, Instant};

use async_io::Timer;
use cairn_model::{AheadBehind, HeadState, RefName};
use freya::prelude::*;

use crate::style::{icons, metrics, tones};

/// Said after an unborn branch's name: `git status`'s "No commits yet".
pub const NO_COMMITS_YET: &str = "no commits yet";
/// Said for a branch whose upstream is configured but gone: git's `[gone]`.
pub const UPSTREAM_GONE: &str = "upstream gone";

/// How long an operation runs before the status box shows it.
pub const PROGRESS_AFTER: Duration = Duration::from_millis(250);

/// An operation running, as the status box shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct Running {
    /// Which operation this is: another is another line, with its own wait.
    pub serial: u64,
    /// Its one name: "Commit 1 file", "Stage 2 Files", "Fetch 'origin'".
    pub name: String,
    pub started: Instant,
    /// The writes queued behind it.
    pub waiting: usize,
    /// git's latest progress line, for a fetch.
    pub progress: Option<String>,
}

/// What the status box's first line says of `running`: its name, and "· N waiting" while writes
/// queue behind it — "Commit 1 file · 1 waiting".
pub fn running_text(running: &Running) -> String {
    let mut text = running.name.clone();
    if running.waiting > 0 {
        text.push_str(&format!(" · {} waiting", running.waiting));
    }
    text
}

/// What the status box's second line says of git's progress `line`: its words up to and
/// including its percentage — "Receiving objects: 45%" of "Receiving objects:  45% (12345/27000),
/// 12.00 MiB | 3.00 MiB/s" — so the line fits the box; git's own alignment padding is closed up.
/// A line with no percentage keeps its words up to its first comma — "remote: Enumerating
/// objects: 5" of "remote: Enumerating objects: 5, done." — or all of them when it has none. The
/// whole line stays in the activity popover.
pub fn progress_text(line: &str) -> String {
    let words = line.split_whitespace().collect::<Vec<_>>().join(" ");
    match words.find('%') {
        Some(at) => words[..=at].to_owned(),
        None => match words.find(',') {
            Some(at) => words[..at].to_owned(),
            None => words,
        },
    }
}

/// Whether an operation that has run `elapsed` is shown yet.
pub fn progress_shown(elapsed: Duration) -> bool {
    elapsed >= PROGRESS_AFTER
}

/// Where the current branch stands against its upstream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tracking {
    /// No upstream, `HEAD` on no branch, or the counts not read yet: nothing is said.
    Untold,
    /// The counts of the branch against its upstream: Push's and Pull's badges.
    Counts(AheadBehind),
    /// The upstream is configured but no such ref exists.
    Gone,
}

/// What the status box says of `HEAD`: the branch's name, `HEAD detached at <short id>`, or
/// an unborn branch's name and that it has no commit.
pub fn head_text(head: &HeadState) -> String {
    match head {
        HeadState::Branch(name) => name.shorthand().to_owned(),
        HeadState::Detached(id) => format!("HEAD detached at {}", id.short().as_str()),
        HeadState::Unborn(name) => format!("{} ({NO_COMMITS_YET})", name.shorthand()),
    }
}

/// The status box's lines: no handler, so equal content compares equal and is not redrawn. The
/// toolbar draws the box around them and the glyph that opens the activity popover.
#[derive(Debug, Clone, PartialEq)]
pub struct StatusBox {
    name: Option<String>,
    head: Option<HeadState>,
    tracking: Tracking,
    running: Vec<Running>,
    key: DiffKey,
}

impl StatusBox {
    /// The box for the repository called `name` — `None` until it is open — and `head` —
    /// `None` until the refs are read.
    pub fn new(name: Option<String>, head: Option<HeadState>) -> Self {
        Self {
            name,
            head,
            tracking: Tracking::Untold,
            running: Vec::new(),
            key: DiffKey::None,
        }
    }

    /// The operations running: the local write, and a fetch.
    pub fn running(mut self, running: Vec<Running>) -> Self {
        self.running = running;
        self
    }

    pub fn tracking(mut self, tracking: Tracking) -> Self {
        self.tracking = tracking;
        self
    }
}

impl KeyExt for StatusBox {
    fn write_key(&mut self) -> &mut DiffKey {
        &mut self.key
    }
}

impl ComponentOwned for StatusBox {
    fn render(self) -> impl IntoElement {
        // One component per set of operations running, so each set waits out its own
        // `PROGRESS_AFTER` from a fresh scope.
        let serials: Vec<u64> = self.running.iter().map(|running| running.serial).collect();
        Lines {
            name: self.name,
            head: self.head,
            gone: self.tracking == Tracking::Gone,
            running: self.running,
        }
        .key(DiffKey::from(&serials))
    }

    fn render_key(&self) -> DiffKey {
        self.key.clone().or(self.default_key())
    }
}

/// The box's two lines: the repository's, or the operations' once one has run
/// [`PROGRESS_AFTER`].
#[derive(PartialEq)]
struct Lines {
    name: Option<String>,
    head: Option<HeadState>,
    gone: bool,
    running: Vec<Running>,
}

impl Lines {
    fn key(self, key: DiffKey) -> KeyedLines {
        KeyedLines { lines: self, key }
    }
}

#[derive(PartialEq)]
struct KeyedLines {
    lines: Lines,
    key: DiffKey,
}

impl Component for KeyedLines {
    fn render_key(&self) -> DiffKey {
        self.key.clone()
    }

    fn render(&self) -> impl IntoElement {
        let lines = &self.lines;
        // Bumped as each operation's wait runs out, so the box is drawn again then.
        let mut woken = use_state(|| 0u32);
        let waits: Vec<Duration> = lines
            .running
            .iter()
            .map(|running| PROGRESS_AFTER.saturating_sub(running.started.elapsed()))
            .filter(|left| !left.is_zero())
            .collect();
        // Tasks of this scope: they end with this set of operations' lines.
        use_hook(move || {
            for left in waits {
                spawn(async move {
                    Timer::after(left).await;
                    *woken.write() += 1;
                });
            }
        });
        let _woken = *woken.read();
        let shown: Vec<&Running> = lines
            .running
            .iter()
            .filter(|running| progress_shown(running.started.elapsed()))
            .collect();
        match shown.as_slice() {
            [] => repository_lines(lines),
            [first, rest @ ..] => {
                let second = first
                    .progress
                    .as_deref()
                    .map(progress_text)
                    .or_else(|| rest.first().map(|other| running_text(other)));
                two_lines(
                    rect()
                        .horizontal()
                        .cross_align(Alignment::center())
                        .spacing(6.)
                        .child(CircularLoader::new().size(metrics::BODY))
                        .child(line_label(running_text(first), true, tones::TEXT))
                        .into(),
                    match second {
                        Some(text) => line_label(text, false, tones::DETAIL_LABEL).into(),
                        None => branch_line(lines),
                    },
                )
            }
        }
    }
}

/// The repository's name over the branch.
fn repository_lines(lines: &Lines) -> Element {
    two_lines(
        match &lines.name {
            Some(name) => line_label(name.clone(), true, tones::TEXT).into(),
            None => rect().into(),
        },
        branch_line(lines),
    )
}

/// The branch glyph and `HEAD` in git's words, and "upstream gone" after a gone upstream.
fn branch_line(lines: &Lines) -> Element {
    // The gone upstream keeps the theme's warning tone, which Fork Dark's table does not
    // name.
    let warning = get_theme_or_default().read().colors().warning;
    let on_branch = match &lines.head {
        Some(HeadState::Branch(_) | HeadState::Unborn(_)) => true,
        Some(HeadState::Detached(_)) | None => false,
    };
    rect()
        .horizontal()
        .cross_align(Alignment::center())
        .spacing(4.)
        .maybe_child(on_branch.then(|| icons::branch(metrics::BODY, tones::TEXT)))
        .maybe_child(
            lines
                .head
                .as_ref()
                .map(|head| line_label(head_text(head), false, tones::TEXT)),
        )
        .maybe_child(
            lines
                .gone
                .then(|| line_label(UPSTREAM_GONE.to_owned(), false, warning)),
        )
        .into()
}

fn two_lines(first: Element, second: Element) -> Element {
    rect()
        .width(Size::fill())
        .height(Size::fill())
        .center()
        .spacing(2.)
        .child(first)
        .child(second)
        .into()
}

fn line_label(text: String, bold: bool, colour: Color) -> Label {
    label()
        .text(text)
        .max_lines(1)
        .text_overflow(TextOverflow::Ellipsis)
        .font_size(metrics::BODY)
        .font_weight(if bold {
            FontWeight::BOLD
        } else {
            FontWeight::NORMAL
        })
        .color(colour)
}

/// The branch `HEAD` is on, when it has a commit: the one whose upstream the badges measure.
pub fn current_branch(head: &HeadState) -> Option<&RefName> {
    match head {
        HeadState::Branch(name) => Some(name),
        HeadState::Detached(_) | HeadState::Unborn(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cairn_model::Oid;

    /// In git's words: a branch by its name, a detached `HEAD` by its short id, an unborn
    /// branch by its name and that it has no commit. Caught by: a full ref name, the long id,
    /// or an unborn branch drawn as an ordinary one.
    #[test]
    fn head_is_named_as_git_names_it() {
        let id = Oid::from_bytes(&[0xab; 20]).unwrap();
        assert_eq!(
            head_text(&HeadState::Branch(RefName::new("refs/heads/feature/x"))),
            "feature/x"
        );
        assert_eq!(
            head_text(&HeadState::Detached(id)),
            "HEAD detached at abababa"
        );
        assert_eq!(
            head_text(&HeadState::Unborn(RefName::new("refs/heads/main"))),
            "main (no commits yet)"
        );
    }

    /// The first line names the operation and the writes queued behind it, and nothing else:
    /// git's progress line is the second. Caught by: the progress line run into the name, or a
    /// count of nothing waiting drawn.
    #[test]
    fn the_running_line_is_the_name_and_what_waits() {
        let running = |waiting, progress: Option<&str>| Running {
            serial: 1,
            name: "Fetch 'origin'".to_owned(),
            started: Instant::now(),
            waiting,
            progress: progress.map(str::to_owned),
        };
        assert_eq!(
            running_text(&running(0, Some("Receiving objects: 40%"))),
            "Fetch 'origin'"
        );
        assert_eq!(
            running_text(&running(2, None)),
            "Fetch 'origin' · 2 waiting"
        );
    }

    /// A progress line is kept to its percentage, its alignment padding closed up; one with no
    /// percentage to its first comma, or whole when it has none. Caught by: the counts and the
    /// throughput left on (wider than the box), the percentage itself cut, git's double space
    /// kept, or a count-only line emptied.
    #[test]
    fn a_progress_line_is_kept_to_its_percentage() {
        assert_eq!(
            progress_text("Receiving objects:  45% (12345/27000), 12.00 MiB | 3.00 MiB/s"),
            "Receiving objects: 45%"
        );
        assert_eq!(
            progress_text("remote: Counting objects: 100% (1234/1234), done."),
            "remote: Counting objects: 100%"
        );
        assert_eq!(
            progress_text("Unpacking objects: 100% (3/3), 268 bytes | 268.00 KiB/s, done."),
            "Unpacking objects: 100%"
        );
        assert_eq!(
            progress_text("remote: Enumerating objects: 5, done."),
            "remote: Enumerating objects: 5"
        );
        assert_eq!(progress_text("From ../remote"), "From ../remote");
        assert_eq!(progress_text("   "), "");
    }
}
