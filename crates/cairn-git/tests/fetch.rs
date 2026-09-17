//! Fetch, end to end: real `git`, a real remote demanding a credential, the
//! built askpass helper, and a real channel answered from a test thread.
//!
//! PRD B3 (a prompt is answered and the fetch succeeds), B4 (a helper or
//! agent that already answers means no prompt at all — the regression that
//! protects L7) and B5 (a refused prompt fails cleanly, once, with nothing
//! left running), each over HTTPS-shaped HTTP and over SSH, because git
//! shares no code between the two. Credentials are generated per fixture and
//! never written into this file.
#![cfg(unix)]

mod fixtures;
mod remotes;

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use cairn_git::ops::{Askpass, GitBinary, GitEnvironment, Invalidated, fetch};
use cairn_git::{Error, Repository};
use cairn_model::PromptKind;

use fixtures::Fixture;
use remotes::askpass::{self, Answers, Askpass as Serving};
use remotes::http::HttpRemote;
use remotes::ssh::{HostKey, SshRemote, Unavailable};

/// An empty repository with `origin` pointing at `url`, and no credential
/// helper inherited from the machine: an empty `credential.helper` entry
/// resets the list git built from the system and global configuration, so a
/// developer's keyring helper is never consulted (or unlocked) by a test.
fn with_origin(url: &str) -> Fixture {
    let fixture = fixtures::unborn();
    fixture.git(&["remote", "add", "origin", url]);
    fixture.git(&["config", "credential.helper", ""]);
    fixture
}

/// The environment git runs with: this process's `PATH`, `home` as `HOME`,
/// `agent` as `SSH_AUTH_SOCK`, and nothing else the machine has.
fn environment(serving: &Serving, home: &Path, agent: Option<&Path>) -> GitEnvironment {
    let home = home.to_owned();
    let agent = agent.map(Path::to_owned);
    GitEnvironment::new(
        move |name| match name {
            "PATH" => std::env::var_os("PATH"),
            "HOME" => Some(home.clone().into_os_string()),
            "SSH_AUTH_SOCK" => agent.clone().map(OsString::from),
            _ => None,
        },
        &Askpass::new(
            askpass::helper_binary(),
            Some(serving.socket_path().to_owned()),
        ),
    )
}

/// Runs one fetch of `origin` to completion, collecting progress.
fn fetch_origin(
    serving: &Serving,
    local: &Fixture,
    home: &Path,
    agent: Option<&Path>,
) -> (Result<cairn_git::ops::Performed, Error>, Vec<String>) {
    let git = GitBinary::discover_with(environment(serving, home, agent))
        .unwrap_or_else(|e| panic!("no usable git: {e}"));
    let repo = Repository::discover(local.path()).unwrap_or_else(|e| panic!("{e}"));
    let operation = serving.channel().begin().unwrap_or_else(|e| panic!("{e}"));
    let mut progress = Vec::new();
    let started = fetch(&git, &repo, "origin", Some(operation.token()))
        .unwrap_or_else(|e| panic!("fetch did not start: {e}"));
    let outcome = started.finish(|line| progress.push(line.to_owned()));
    (outcome, progress)
}

fn head_of(fixture: &Fixture) -> String {
    fixture.rev_list()[0].clone()
}

fn fetched_main(local: &Fixture) -> String {
    local
        .git(&["rev-parse", "refs/remotes/origin/main"])
        .trim()
        .to_owned()
}

fn answers_with(username: String, password: String) -> Answers {
    Box::new(move |prompt| match PromptKind::of(prompt) {
        PromptKind::Username => Some(username.clone()),
        PromptKind::Password | PromptKind::Passphrase => Some(password.clone()),
        PromptKind::Confirmation => Some(PromptKind::ACCEPTED.to_owned()),
        PromptKind::Other => None,
    })
}

fn refusing() -> Answers {
    Box::new(|_| None)
}

/// Answers the username and refuses the password — the only shape in which git's
/// credential machinery could ask twice, since it retries a credential the server
/// rejected and a refusal at the first ask never gets that far.
fn refusing_only_the_password(username: String) -> Answers {
    Box::new(move |prompt| match PromptKind::of(prompt) {
        PromptKind::Username => Some(username.clone()),
        PromptKind::Password
        | PromptKind::Passphrase
        | PromptKind::Confirmation
        | PromptKind::Other => None,
    })
}

/// The ssh fixture could not be built. Named on stderr (which `cargo test`
/// shows only with `--nocapture` or on failure), and a FAILURE where
/// `CAIRN_REQUIRE_SSH_FIXTURE` is set — which is how a CI job that provides
/// `sshd` turns a skipped criterion from an invisible `ok` into a red run.
fn skipped(test: &str, why: &Unavailable) {
    if std::env::var_os("CAIRN_REQUIRE_SSH_FIXTURE").is_some() {
        panic!(
            "{test}: the ssh fixture is required here but unavailable: {}",
            why.0
        );
    }
    eprintln!(
        "SKIPPED {test}: the ssh fixture is unavailable here: {}",
        why.0
    );
}

fn no_secret_in(text: &str, secret: &str, what: &str) {
    assert!(
        !text.contains(secret),
        "{what} carries the secret: {text:?}"
    );
}

// ── HTTP ────────────────────────────────────────────────────────────────────

/// PRD B3: a username and a password, each asked once through the helper,
/// and the fetch brings the remote's history across.
#[test]
fn a_fetch_over_http_prompts_for_each_half_of_the_credential_and_succeeds() {
    let source = fixtures::braided(6);
    let remote = HttpRemote::of(&source);
    let serving = Serving::serving(answers_with(
        remote.username().to_owned(),
        remote.password().to_owned(),
    ));
    let local = with_origin(remote.url());

    let (outcome, progress) = fetch_origin(&serving, &local, local.path(), None);
    let performed = outcome.unwrap_or_else(|e| panic!("the fetch failed: {e}"));

    assert_eq!(performed.description(), "fetched origin");
    assert_eq!(
        performed.invalidated(),
        Invalidated::refs().and(Invalidated::objects())
    );
    assert_eq!(performed.acknowledged(), None, "a fetch confirms nothing");
    assert_eq!(fetched_main(&local), head_of(&source));
    assert_eq!(
        serving.prompts(),
        [
            format!(
                "Username for '{}': ",
                remote.url().trim_end_matches("/repo.git")
            ),
            format!(
                "Password for '{}': ",
                remote
                    .url()
                    .trim_end_matches("/repo.git")
                    .replace("http://", &format!("http://{}@", remote.username()))
            ),
        ],
        "git did not ask exactly for a username and then a password"
    );
    assert!(
        remote
            .seen()
            .iter()
            .any(|seen| seen.authorized_as.as_deref() == Some(remote.username())),
        "the credential the helper answered never reached the server: {:?}",
        remote.seen()
    );
    assert!(!progress.is_empty(), "no progress was reported");
    for line in &progress {
        no_secret_in(line, remote.password(), "a progress line");
    }
}

/// PRD B4, the criterion that protects L7: a `credential.helper` the user
/// configured answers, and Cairn's helper is never asked. The stub helper is
/// the only thing that knows the password, and the askpass channel REFUSES
/// anything it is asked — so if Cairn prompted, the fetch would fail and the
/// prompt list would not be empty; the test cannot pass by the helper being
/// skipped either, because the server records which username authorised.
#[test]
fn a_credential_helper_that_answers_means_no_prompt_at_all() {
    let source = fixtures::braided(4);
    let remote = HttpRemote::of(&source);
    let serving = Serving::serving(refusing());
    let local = with_origin(remote.url());

    // The helper is configured the way a real user has one: in ~/.gitconfig.
    let home = local.path().join("home");
    std::fs::create_dir_all(&home).unwrap_or_else(|e| panic!("{e}"));
    let helper = home.join("credential-helper.sh");
    std::fs::write(
        &helper,
        format!(
            "#!/bin/sh\nif [ \"$1\" = get ]; then printf 'username=%s\\npassword=%s\\n' '{}' '{}'; fi\n",
            remote.username(),
            remote.password()
        ),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o700))
            .unwrap_or_else(|e| panic!("{e}"));
    }
    // Reset first, so nothing from the machine's system configuration runs.
    std::fs::write(
        home.join(".gitconfig"),
        format!(
            "[credential]\n\thelper = \n\thelper = {}\n",
            helper.display()
        ),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    // The local reset `with_origin` wrote would also discard the global helper.
    local.git(&["config", "--unset", "credential.helper"]);

    let (outcome, _) = fetch_origin(&serving, &local, &home, None);
    let performed = outcome.unwrap_or_else(|e| panic!("the fetch failed: {e}"));

    assert_eq!(
        serving.prompts(),
        Vec::<String>::new(),
        "Cairn prompted even though the user's credential helper answers (L7, PRD B4)"
    );
    assert_eq!(fetched_main(&local), head_of(&source));
    assert!(performed.invalidated().refs);
    assert!(
        remote
            .seen()
            .iter()
            .any(|seen| seen.authorized_as.as_deref() == Some(remote.username())),
        "nothing authorised at the server, so the helper was not what answered"
    );
}

/// PRD B5: the user declines, and the fetch fails at once — one prompt, no
/// second ask, no hang, a message naming what git could not read, and no
/// secret anywhere in what a user would see.
#[test]
fn refusing_the_http_prompt_fails_the_fetch_cleanly_and_asks_nothing_again() {
    let source = fixtures::braided(4);
    let remote = HttpRemote::of(&source);
    let serving = Serving::serving(refusing());
    let local = with_origin(remote.url());

    let started = Instant::now();
    let (outcome, progress) = fetch_origin(&serving, &local, local.path(), None);
    let error = match outcome {
        Err(error) => error,
        Ok(performed) => panic!("a refused credential fetched anyway: {performed:?}"),
    };
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "a refusal took {:?}: something waited",
        started.elapsed()
    );
    match &error {
        Error::GitFailed { stderr, .. } => assert!(
            stderr.contains("terminal prompts disabled") || stderr.contains("could not read"),
            "the failure does not say what could not be read: {stderr}"
        ),
        other => panic!("expected git to fail, got {other:?}"),
    }
    assert_eq!(
        serving.prompts().len(),
        1,
        "git asked more than once after a refusal: {:?}",
        serving.prompts()
    );
    no_secret_in(
        &error.to_string(),
        remote.password(),
        "the error shown to the user",
    );
    for line in &progress {
        no_secret_in(line, remote.password(), "a progress line");
    }
    assert!(
        local
            .git(&["for-each-ref", "refs/remotes"])
            .trim()
            .is_empty(),
        "a failed fetch moved a ref"
    );
    let left =
        askpass::wait_for_no_process_pointed_at(serving.socket_path(), Duration::from_secs(5));
    assert!(
        left.is_empty(),
        "processes left behind after the refusal: {left:?}"
    );
}

/// PRD B5, the half a refusal at the FIRST prompt cannot reach. `refusing()` makes
/// git fail at the username, so it never presents a credential and never gets one
/// rejected — but git retries a rejected credential, which is exactly where a second
/// dialog would appear. Here the username is answered and the password refused: the
/// ask must stop at two, and the fetch must fail rather than loop.
#[test]
fn refusing_the_password_after_answering_the_username_asks_for_neither_again() {
    let source = fixtures::braided(4);
    let remote = HttpRemote::of(&source);
    let serving = Serving::serving(refusing_only_the_password(remote.username().to_owned()));
    let local = with_origin(remote.url());

    let started = Instant::now();
    let (outcome, progress) = fetch_origin(&serving, &local, local.path(), None);
    let error = match outcome {
        Err(error) => error,
        Ok(performed) => panic!("a refused password fetched anyway: {performed:?}"),
    };
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "a refusal took {:?}: something waited or retried",
        started.elapsed()
    );

    let asked = serving.prompts();
    assert_eq!(
        asked.len(),
        2,
        "git asked again after the password was refused: {asked:?}"
    );
    assert!(
        matches!(PromptKind::of(&asked[0]), PromptKind::Username)
            && matches!(PromptKind::of(&asked[1]), PromptKind::Password),
        "the two asks were not the username and then the password: {asked:?}"
    );

    no_secret_in(
        &error.to_string(),
        remote.password(),
        "the error shown to the user",
    );
    for line in &progress {
        no_secret_in(line, remote.password(), "a progress line");
    }
    assert!(
        local
            .git(&["for-each-ref", "refs/remotes"])
            .trim()
            .is_empty(),
        "a fetch that never authenticated moved a ref"
    );
    let left =
        askpass::wait_for_no_process_pointed_at(serving.socket_path(), Duration::from_secs(5));
    assert!(
        left.is_empty(),
        "processes left behind after the refusal: {left:?}"
    );
}

/// PRD R4.3, the cancel: git is killed while it waits on a prompt nobody has
/// answered; the wait ends promptly, the outcome says cancelled, and once the
/// pending prompt is refused (as closing the dialog does) nothing is left.
#[test]
fn cancelling_a_fetch_that_is_waiting_on_a_prompt_kills_git_and_leaves_nothing_behind() {
    let source = fixtures::braided(4);
    let remote = HttpRemote::of(&source);
    // Each prompt waits until told what to answer; the test holds it open.
    let (tell, told) = std::sync::mpsc::channel::<Option<String>>();
    let (arrived, prompt_arrived) = std::sync::mpsc::channel::<()>();
    let told = Mutex::new(told);
    let serving = Serving::serving(Box::new(move |_| {
        let _ = arrived.send(());
        told.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .recv()
            .unwrap_or(None)
    }));
    let local = with_origin(remote.url());

    let git = GitBinary::discover_with(environment(&serving, local.path(), None))
        .unwrap_or_else(|e| panic!("no usable git: {e}"));
    let repo = Repository::discover(local.path()).unwrap_or_else(|e| panic!("{e}"));
    let operation = serving.channel().begin().unwrap_or_else(|e| panic!("{e}"));
    let started = fetch(&git, &repo, "origin", Some(operation.token()))
        .unwrap_or_else(|e| panic!("fetch did not start: {e}"));
    let canceller = started.canceller();

    let cancelling = std::thread::spawn(move || {
        prompt_arrived
            .recv_timeout(Duration::from_secs(20))
            .unwrap_or_else(|_| panic!("git never asked for a credential"));
        let waited = Instant::now();
        canceller.cancel();
        waited
    });
    let outcome = started.finish(|_| {});
    let cancelled_at = cancelling
        .join()
        .unwrap_or_else(|_| panic!("the cancelling thread panicked"));
    // Under the two-second grace period: a git that acted on the SIGTERM, not one that
    // had to be SIGKILLed.
    assert!(
        cancelled_at.elapsed() < Duration::from_secs(2),
        "the cancel took {:?} to end the wait: git did not act on SIGTERM",
        cancelled_at.elapsed()
    );
    assert!(
        matches!(outcome, Err(Error::GitCancelled { .. })),
        "a cancelled fetch reported {outcome:?}"
    );
    // The dialog closing is what refuses the prompt the helper is still waiting on.
    let _ = tell.send(None);
    drop(operation);
    let left =
        askpass::wait_for_no_process_pointed_at(serving.socket_path(), Duration::from_secs(5));
    assert!(
        left.is_empty(),
        "processes left behind after the cancel: {left:?}"
    );
}

/// Issue #19, end to end over a real git: a cancel reports every `*.lock` left
/// under the git directory once git is gone, and nothing when there is none.
/// git hangs on a remote that accepts the connection and never answers, so it
/// is mid-transport and holds no lock of its own when the cancel lands; the
/// locks here are planted, as a crash or an earlier `SIGKILL` would leave
/// them, and the cancel is sent only once git has connected, so it is a
/// running git that is ended. `SIGTERM` reaches git itself (its curl
/// transport runs in-process), which is the signal it removes its own locks
/// on; that it exits promptly on it is what keeps the bound below.
#[test]
fn a_cancel_names_the_lock_files_left_under_the_git_directory_and_only_those() {
    let unanswering = std::net::TcpListener::bind("127.0.0.1:0").unwrap_or_else(|e| panic!("{e}"));
    let port = unanswering
        .local_addr()
        .unwrap_or_else(|e| panic!("{e}"))
        .port();
    let local = with_origin(&format!("http://127.0.0.1:{port}/never.git"));
    let git_dir = local.path().join(".git");
    let planted: Vec<PathBuf> = ["packed-refs.lock", "refs/remotes/origin/main.lock"]
        .into_iter()
        .map(|relative| {
            let path = git_dir.join(relative);
            std::fs::create_dir_all(path.parent().unwrap_or(&path))
                .unwrap_or_else(|e| panic!("{e}"));
            std::fs::write(&path, b"").unwrap_or_else(|e| panic!("{e}"));
            std::fs::canonicalize(&path).unwrap_or(path)
        })
        .collect();
    let serving = Serving::serving(Box::new(|_| None));
    let git = GitBinary::discover_with(environment(&serving, local.path(), None))
        .unwrap_or_else(|e| panic!("no usable git: {e}"));
    let repo = Repository::discover(local.path()).unwrap_or_else(|e| panic!("{e}"));

    // Cancelling the fetch once git is connected and waiting for an answer that never comes;
    // the listener is held open (never read, never replied to) until the cancel has landed.
    let cancel_once_connected = |started: cairn_git::ops::FetchInProgress| {
        let canceller = started.canceller();
        let listener = unanswering.try_clone().unwrap_or_else(|e| panic!("{e}"));
        let cancelling = std::thread::spawn(move || {
            let connection = listener.accept().map(|(stream, _)| stream);
            let waited = Instant::now();
            canceller.cancel();
            (connection, waited)
        });
        let outcome = started.finish(|_| {});
        let (connection, cancelled_at) = cancelling.join().unwrap_or_else(|_| panic!("join"));
        drop(connection.unwrap_or_else(|e| panic!("git never connected: {e}")));
        // Under the two-second grace period: only a git that acted on the SIGTERM ends
        // this soon, since one that ignored it is SIGKILLed no earlier than that.
        assert!(
            cancelled_at.elapsed() < Duration::from_secs(2),
            "the cancel took {:?} to end the wait: git did not act on SIGTERM",
            cancelled_at.elapsed()
        );
        match outcome {
            Err(Error::GitCancelled { stranded_locks, .. }) => stranded_locks
                .into_iter()
                .map(|path| std::fs::canonicalize(&path).unwrap_or(path))
                .collect::<Vec<PathBuf>>(),
            other => panic!("a cancelled fetch reported {other:?}"),
        }
    };

    let started = fetch(&git, &repo, "origin", None).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        cancel_once_connected(started),
        planted,
        "the cancel did not name exactly the lock files under the git directory"
    );

    // Without them, a cancel that caught git mid-transport strands nothing.
    for path in &planted {
        std::fs::remove_file(path).unwrap_or_else(|e| panic!("{e}"));
    }
    let started = fetch(&git, &repo, "origin", None).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        cancel_once_connected(started),
        Vec::<PathBuf>::new(),
        "a cancel that left no lock behind reported one"
    );
}

// ── Ref tips ────────────────────────────────────────────────────────────────

/// What the worker compares before and after a fetch: every ref's id as git
/// wrote it, gaining an entry when a ref is made and changing when one moves.
/// Over a fixture, since the Cairn checkout's own refs are whatever the
/// machine (or a CI checkout, detached with no branch) has.
#[test]
fn ref_tips_follow_the_refs_git_writes() {
    let fixture = fixtures::braided(3);
    let repo = Repository::discover(fixture.path()).unwrap_or_else(|e| panic!("{e}"));
    let git_says = |reference: &str| fixture.git(&["rev-parse", reference]).trim().to_owned();
    let tip_of = |reference: &str| {
        repo.ref_tips()
            .unwrap_or_else(|e| panic!("{e}"))
            .iter()
            .find(|(name, _)| name.as_str() == reference)
            .map(|(_, oid)| oid.to_string())
    };

    assert_eq!(tip_of("refs/heads/main"), Some(git_says("refs/heads/main")));
    assert_eq!(tip_of("refs/heads/side"), Some(git_says("refs/heads/side")));
    assert_eq!(tip_of("refs/tags/marker"), None);

    fixture.git(&["tag", "marker", "refs/heads/side"]);
    assert_eq!(
        tip_of("refs/tags/marker"),
        Some(git_says("refs/heads/side")),
        "a ref git made is not in the tips"
    );

    let before = tip_of("refs/heads/side");
    fixture.git(&["update-ref", "refs/heads/side", "refs/heads/main"]);
    assert_eq!(
        tip_of("refs/heads/side"),
        Some(git_says("refs/heads/main")),
        "a ref git moved kept its old tip"
    );
    assert_ne!(
        tip_of("refs/heads/side"),
        before,
        "the move changed nothing"
    );
}

// ── Pruning ─────────────────────────────────────────────────────────────────

/// A clone of `source` that has fetched once and then grown a remote-tracking
/// ref the remote never had and a tag of its own: what `fetch.prune` would
/// remove, and what nothing may. The remote is a local path — nothing here is
/// about credentials, and the channel refuses anything asked.
struct Pruning {
    local: Fixture,
    serving: Serving,
    tip: String,
}

impl Pruning {
    fn new(source: &Fixture) -> Self {
        let serving = Serving::serving(refusing());
        let local = with_origin(&source.path().display().to_string());
        local.git(&["fetch", "--quiet", "origin"]);
        let tip = fetched_main(&local);
        let pruning = Self {
            local,
            serving,
            tip,
        };
        pruning.restore_stale_refs();
        pruning
    }

    /// The stale remote-tracking ref and the local tag, (re)made after a fetch
    /// that removed either.
    fn restore_stale_refs(&self) {
        self.local
            .git(&["update-ref", "refs/remotes/origin/gone", &self.tip]);
        self.local.git(&["tag", "-f", "local-only", &self.tip]);
    }

    fn configure(&self, settings: &[(&str, &str)]) {
        for (setting, value) in settings {
            self.local.git(&["config", setting, value]);
        }
    }

    fn unset(&self, settings: &[&str]) {
        for setting in settings {
            self.local.git(&["config", "--unset", setting]);
        }
    }

    /// What is still there: `gone` and/or `local-only`, as git lists them.
    fn stale(&self) -> String {
        self.local.git(&[
            "for-each-ref",
            "--format=%(refname:short)",
            "refs/remotes/origin/gone",
            "refs/tags/local-only",
        ])
    }

    fn cairn_fetches(&self) -> String {
        let (outcome, _) = fetch_origin(&self.serving, &self.local, self.local.path(), None);
        outcome.unwrap_or_else(|e| panic!("the fetch failed: {e}"));
        assert_eq!(self.serving.prompts(), Vec::<String>::new());
        self.stale()
    }

    /// The control: plain `git fetch`, same repository, same configuration.
    fn git_fetches(&self) -> String {
        self.local.git(&["fetch", "--quiet", "origin"]);
        self.stale()
    }
}

const BOTH: &str = "origin/gone\nlocal-only\n";
const TAG_ONLY: &str = "local-only\n";

/// Issue #17 (a): `fetch.prune` is honoured as `git fetch` honours it — the
/// remote-tracking ref the remote never had goes — and the tag stays.
#[test]
fn fetch_prune_on_prunes_a_stale_remote_tracking_ref_as_git_does() {
    let source = fixtures::braided(4);
    let pruning = Pruning::new(&source);
    pruning.configure(&[("fetch.prune", "true")]);
    assert_eq!(pruning.stale(), BOTH);
    assert_eq!(
        pruning.cairn_fetches(),
        TAG_ONLY,
        "fetch.prune was not honoured, or the tag was pruned"
    );
    pruning.restore_stale_refs();
    assert_eq!(
        pruning.git_fetches(),
        TAG_ONLY,
        "plain git differs from Cairn here"
    );
}

/// Issue #17 (b): with `fetch.prune` unset nothing is pruned, by Cairn or by git.
#[test]
fn fetch_prune_unset_keeps_a_stale_remote_tracking_ref_as_git_does() {
    let source = fixtures::braided(4);
    let pruning = Pruning::new(&source);
    assert_eq!(
        pruning.cairn_fetches(),
        BOTH,
        "Cairn pruned with nothing configured"
    );
    assert_eq!(
        pruning.git_fetches(),
        BOTH,
        "plain git differs from Cairn here"
    );
}

/// Issue #17 (c): a local tag survives whatever `fetch.pruneTags` and
/// `remote.<name>.pruneTags` say, while the remote-tracking ref is still
/// pruned; the control shows plain git deleting the tag under the same
/// configuration, so the tag survived because of `--no-prune-tags`.
#[test]
fn a_local_tag_survives_whatever_prune_tags_says() {
    let source = fixtures::braided(4);
    let pruning = Pruning::new(&source);
    pruning.configure(&[
        ("fetch.prune", "true"),
        ("fetch.pruneTags", "true"),
        ("remote.origin.pruneTags", "true"),
    ]);
    assert_eq!(pruning.cairn_fetches(), TAG_ONLY, "the tag was pruned");
    pruning.restore_stale_refs();
    assert_eq!(
        pruning.git_fetches(),
        "",
        "plain git did not prune the tag here, so the assertion above decided nothing"
    );
}

/// Issue #17 (d): `remote.<name>.prune` overrides `fetch.prune` in both
/// directions, for Cairn as for git.
#[test]
fn remote_prune_overrides_fetch_prune_both_ways_as_git_does() {
    let source = fixtures::braided(4);
    let pruning = Pruning::new(&source);

    pruning.configure(&[("fetch.prune", "true"), ("remote.origin.prune", "false")]);
    assert_eq!(
        pruning.cairn_fetches(),
        BOTH,
        "remote.origin.prune=false did not win"
    );
    assert_eq!(pruning.git_fetches(), BOTH);

    pruning.configure(&[("fetch.prune", "false"), ("remote.origin.prune", "true")]);
    assert_eq!(
        pruning.cairn_fetches(),
        TAG_ONLY,
        "remote.origin.prune=true did not win"
    );
    pruning.restore_stale_refs();
    assert_eq!(pruning.git_fetches(), TAG_ONLY);

    pruning.unset(&["fetch.prune", "remote.origin.prune"]);
    pruning.restore_stale_refs();
    assert_eq!(
        pruning.cairn_fetches(),
        BOTH,
        "unsetting both did not stop pruning"
    );
}

/// A `git` on a `PATH` of its own that answers `--version` and, for anything
/// else, leaves a file saying it ran and then hangs — so "no process started"
/// is a file that is not there, checked after the time a start would take,
/// rather than a race against a real git that had not written yet.
struct RecordingGit {
    directory: PathBuf,
}

impl RecordingGit {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let directory = std::env::temp_dir().join(format!(
            "cairn-recording-git-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap_or_else(|e| panic!("{e}"));
        // Its PATH is this directory alone, so the system's comes first for `touch`.
        let script = "#!/bin/sh\nif [ \"$1\" = --version ]; then echo 'git version 2.30.0'; exit 0; fi\n\
                      PATH=/usr/bin:/bin; touch \"$(dirname \"$0\")/ran\"; sleep 5\n";
        let git = directory.join("git");
        std::fs::write(&git, script).unwrap_or_else(|e| panic!("{e}"));
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&git, std::fs::Permissions::from_mode(0o755))
            .unwrap_or_else(|e| panic!("{e}"));
        Self { directory }
    }

    fn environment(&self, serving: &Serving, home: &Path) -> GitEnvironment {
        let directory = self.directory.clone();
        let home = home.to_owned();
        GitEnvironment::new(
            move |name| match name {
                "PATH" => Some(directory.clone().into_os_string()),
                "HOME" => Some(home.clone().into_os_string()),
                _ => None,
            },
            &Askpass::new(
                askpass::helper_binary(),
                Some(serving.socket_path().to_owned()),
            ),
        )
    }

    /// Whether the stub was ever run as anything but `--version`, after a wait
    /// long enough for a spawn to have got to its first line.
    fn ran(&self) -> bool {
        std::thread::sleep(Duration::from_millis(300));
        self.directory.join("ran").exists()
    }
}

impl Drop for RecordingGit {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

/// What a refused fetch reports, and that nothing ran: the stub `git` would
/// have said so.
fn refused_before_any_git_ran(
    local: &Fixture,
    serving: &Serving,
    expected_setting: &str,
    expected_write: cairn_git::RefusedWrite,
) {
    let recording = RecordingGit::new();
    let git = GitBinary::discover_with(recording.environment(serving, local.path()))
        .unwrap_or_else(|e| panic!("no usable git: {e}"));
    let repo = Repository::discover(local.path()).unwrap_or_else(|e| panic!("{e}"));
    let outcome = fetch(&git, &repo, "origin", None);
    match outcome {
        Err(Error::FetchRefused {
            remote,
            setting,
            write,
        }) => {
            assert_eq!(remote, "origin");
            assert_eq!(setting, expected_setting);
            assert_eq!(write, expected_write);
        }
        Ok(_) => panic!("the fetch was not refused under {expected_setting}"),
        Err(other) => panic!("refused for the wrong reason under {expected_setting}: {other}"),
    }
    assert!(
        !recording.ran(),
        "a git process was started under {expected_setting} before the refusal"
    );
}

/// The recording stub decides something: a fetch that is NOT refused starts it,
/// and the file appears. Without this the assertion above could pass on a stub
/// that never records.
#[test]
fn the_recording_stub_reports_a_fetch_that_was_allowed_to_start() {
    let source = fixtures::braided(2);
    let serving = Serving::serving(refusing());
    let local = with_origin(&source.path().display().to_string());
    let recording = RecordingGit::new();
    let git = GitBinary::discover_with(recording.environment(&serving, local.path()))
        .unwrap_or_else(|e| panic!("no usable git: {e}"));
    let repo = Repository::discover(local.path()).unwrap_or_else(|e| panic!("{e}"));
    let started = fetch(&git, &repo, "origin", None).unwrap_or_else(|e| panic!("{e}"));
    assert!(recording.ran(), "the allowed fetch did not start the stub");
    let canceller = started.canceller();
    canceller.cancel();
    assert!(matches!(
        started.finish(|_| {}),
        Err(Error::GitCancelled { .. })
    ));
}

/// Issue #17 (e): a remote whose refspecs would write local branches — a
/// mirror, `remote.<name>.mirror`, or any `refs/heads/` destination — is
/// refused with the setting quoted, and no git process runs. The repository
/// is opened BEFORE the setting is written, so a refspec added in a terminal
/// after Cairn opened the repository is still what refuses the fetch.
#[test]
fn a_refspec_that_writes_local_branches_is_refused_before_git_runs() {
    let source = fixtures::braided(3);
    let serving = Serving::serving(refusing());
    let message = {
        let local = with_origin(&source.path().display().to_string());
        let recording = RecordingGit::new();
        let git = GitBinary::discover_with(recording.environment(&serving, local.path()))
            .unwrap_or_else(|e| panic!("no usable git: {e}"));
        let repo = Repository::discover(local.path()).unwrap_or_else(|e| panic!("{e}"));
        local.git(&["config", "remote.origin.fetch", "+refs/*:refs/*"]);
        let error = match fetch(&git, &repo, "origin", None) {
            Err(error) => error,
            Ok(_) => panic!("a mirror refspec written after the repository was opened was fetched"),
        };
        assert!(!recording.ran(), "a git process was started");
        error.to_string()
    };
    assert!(
        message.contains("+refs/*:refs/*") && message.contains("local branches"),
        "the refusal did not name the refspec and what it would write: {message}"
    );

    for (settings, expected_setting, write) in [
        (
            vec![("remote.origin.fetch", "+refs/heads/*:refs/heads/*")],
            "remote.origin.fetch = +refs/heads/*:refs/heads/*",
            cairn_git::RefusedWrite::LocalBranches,
        ),
        (
            vec![("remote.origin.fetch", "refs/heads/main:refs/heads/main")],
            "remote.origin.fetch = refs/heads/main:refs/heads/main",
            cairn_git::RefusedWrite::LocalBranches,
        ),
        (
            vec![("remote.origin.mirror", "true")],
            "remote.origin.mirror = true",
            cairn_git::RefusedWrite::Mirror,
        ),
    ] {
        let local = with_origin(&source.path().display().to_string());
        for (setting, value) in &settings {
            local.git(&["config", setting, value]);
        }
        refused_before_any_git_ran(&local, &serving, expected_setting, write);
    }
}

/// Issue #17: a remote whose own refspecs write `refs/tags/` is refused while
/// pruning is on — `--no-prune-tags` withholds only the refspec git would add,
/// not one the user configured — and fetched, tag intact, when it is off.
/// "On" is read as git reads it, `remote.<name>.prune` over `fetch.prune`, and
/// the refusal names the setting that decided.
#[test]
fn a_tag_refspec_is_refused_under_prune_and_fetched_without_it() {
    let source = fixtures::braided(3);
    let serving = Serving::serving(refusing());
    let refspec = "+refs/tags/*:refs/tags/*";

    let local = with_origin(&source.path().display().to_string());
    local.git(&["config", "--add", "remote.origin.fetch", refspec]);
    local.git(&["config", "fetch.prune", "true"]);
    refused_before_any_git_ran(
        &local,
        &serving,
        &format!("remote.origin.fetch = {refspec} with fetch.prune = true"),
        cairn_git::RefusedWrite::LocalTags,
    );

    // The remote's own setting wins over fetch.prune, in both directions.
    local.git(&["config", "remote.origin.prune", "false"]);
    let (outcome, _) = fetch_origin(&serving, &local, local.path(), None);
    outcome.unwrap_or_else(|e| panic!("remote.origin.prune=false did not win: {e}"));
    local.git(&["config", "fetch.prune", "false"]);
    local.git(&["config", "remote.origin.prune", "true"]);
    refused_before_any_git_ran(
        &local,
        &serving,
        &format!("remote.origin.fetch = {refspec} with remote.origin.prune = true"),
        cairn_git::RefusedWrite::LocalTags,
    );

    local.git(&["config", "--unset", "fetch.prune"]);
    local.git(&["config", "--unset", "remote.origin.prune"]);
    let (outcome, _) = fetch_origin(&serving, &local, local.path(), None);
    outcome.unwrap_or_else(|e| panic!("a tag refspec without pruning was refused: {e}"));
    assert_eq!(fetched_main(&local), head_of(&source));
}

/// The check reads configuration as the child git will: a `GIT_CONFIG_*` in
/// Cairn's own environment never reaches the child (the environment is built,
/// not inherited), so it must not steer the check either. Pinned through both
/// routes gix has for such variables — `GIT_CONFIG_COUNT` with a key/value
/// pair, which its config-from-environment permission gates, and
/// `GIT_CONFIG_GLOBAL` naming a file, which its `GIT_*` permission gates —
/// each turning pruning on for the check while git prunes nothing, so a check
/// that honoured either would refuse the tag refspec below.
#[test]
fn the_refspec_check_ignores_config_from_cairns_own_environment() {
    // `std::env::set_var` is unsafe in the 2024 edition and unsafe is forbidden, so the
    // variables are set for a child process that runs this check: the test binary itself,
    // filtered to the inner test below.
    let inner = "the_refspec_check_ignores_config_from_cairns_own_environment_inner";
    let exe = std::env::current_exe().unwrap_or_else(|e| panic!("{e}"));
    let global = std::env::temp_dir().join(format!(
        "cairn-refspec-check-global-{}.gitconfig",
        std::process::id()
    ));
    std::fs::write(&global, "[fetch]\n\tprune = true\n").unwrap_or_else(|e| panic!("{e}"));
    let output = std::process::Command::new(exe)
        .args(["--exact", inner, "--include-ignored", "--nocapture"])
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "fetch.prune")
        .env("GIT_CONFIG_VALUE_0", "true")
        .env("GIT_CONFIG_GLOBAL", &global)
        .env("CAIRN_TEST_INNER", "1")
        .output();
    let _ = std::fs::remove_file(&global);
    let output = output.unwrap_or_else(|e| panic!("{e}"));
    assert!(
        output.status.success(),
        "the inner test failed under GIT_CONFIG_*:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("1 passed"),
        "the inner test did not run:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
}

/// The inner half of the test above; ignored so it runs only under its parent, where
/// `GIT_CONFIG_*` is set to turn pruning on if anything read it.
#[test]
#[ignore = "run by the_refspec_check_ignores_config_from_cairns_own_environment"]
fn the_refspec_check_ignores_config_from_cairns_own_environment_inner() {
    assert_eq!(
        std::env::var("GIT_CONFIG_COUNT").as_deref(),
        Ok("1"),
        "not running under the parent test"
    );
    assert!(
        std::env::var_os("GIT_CONFIG_GLOBAL").is_some_and(|file| {
            std::fs::read_to_string(file).is_ok_and(|text| text.contains("prune = true"))
        }),
        "the parent did not point GIT_CONFIG_GLOBAL at a file that turns pruning on"
    );
    let source = fixtures::braided(2);
    let serving = Serving::serving(refusing());
    let local = with_origin(&source.path().display().to_string());
    local.git(&[
        "config",
        "--add",
        "remote.origin.fetch",
        "+refs/tags/*:refs/tags/*",
    ]);
    // The real git in this fixture's helper inherits this process's GIT_CONFIG_* and would
    // prune; the git Cairn runs does not, and neither may the check.
    let recording = RecordingGit::new();
    let git = GitBinary::discover_with(recording.environment(&serving, local.path()))
        .unwrap_or_else(|e| panic!("no usable git: {e}"));
    let repo = Repository::discover(local.path()).unwrap_or_else(|e| panic!("{e}"));
    match fetch(&git, &repo, "origin", None) {
        Ok(started) => {
            assert!(recording.ran());
            started.canceller().cancel();
            let _ = started.finish(|_| {});
        }
        Err(error) => panic!(
            "the check read fetch.prune from Cairn's own GIT_CONFIG_*, which the child git \
             never sees: {error}"
        ),
    }
}

/// A remote whose configuration gix cannot read is `Error::RemoteConfig`, and
/// no process starts: git would refuse the refspec itself, but the fetch must
/// not reach it on the strength of a check that could not run.
#[test]
fn an_unreadable_remote_configuration_is_reported_and_starts_nothing() {
    let source = fixtures::braided(2);
    let serving = Serving::serving(refusing());
    let local = with_origin(&source.path().display().to_string());
    local.git(&[
        "config",
        "remote.origin.fetch",
        "refs/heads/*:refs/remotes/origin",
    ]);
    let recording = RecordingGit::new();
    let git = GitBinary::discover_with(recording.environment(&serving, local.path()))
        .unwrap_or_else(|e| panic!("no usable git: {e}"));
    let repo = Repository::discover(local.path()).unwrap_or_else(|e| panic!("{e}"));
    match fetch(&git, &repo, "origin", None) {
        Err(Error::RemoteConfig { remote, source }) => {
            assert_eq!(remote, "origin");
            assert!(!source.to_string().is_empty());
        }
        other => panic!("expected the configuration error, got {other:?}"),
    }
    assert!(!recording.ran(), "a git process was started");
}

// ── SSH ─────────────────────────────────────────────────────────────────────

/// PRD B3 over ssh: the key's passphrase is asked once, through the helper,
/// even though this test process has a terminal or not (`SSH_ASKPASS_REQUIRE=force`).
#[test]
fn a_fetch_over_ssh_prompts_for_the_key_passphrase_and_succeeds() {
    let source = fixtures::braided(6);
    let remote = match SshRemote::of(&source, HostKey::Known) {
        Ok(remote) => remote,
        Err(why) => {
            return skipped(
                "a_fetch_over_ssh_prompts_for_the_key_passphrase_and_succeeds",
                &why,
            );
        }
    };
    let serving = Serving::serving(answers_with(String::new(), remote.passphrase().to_owned()));
    let local = with_origin(remote.url());
    local.git(&["config", "core.sshCommand", &remote.ssh_command()]);

    let (outcome, progress) = fetch_origin(&serving, &local, remote.home(), None);
    let performed = outcome.unwrap_or_else(|e| panic!("the fetch failed: {e}"));

    assert!(performed.invalidated().refs && performed.invalidated().objects);
    assert_eq!(fetched_main(&local), head_of(&source));
    assert_eq!(serving.prompts(), [remote.passphrase_prompt()]);
    assert!(!progress.is_empty(), "no progress was reported");
    for line in &progress {
        no_secret_in(line, remote.passphrase(), "a progress line");
    }
}

/// An unknown host key is a question, not a secret: it reaches the helper as
/// ssh's whole confirmation text and is answered `yes` before the passphrase
/// is asked. The dialog's distinct rendering is what this path needs.
#[test]
fn an_unknown_host_key_is_confirmed_through_the_helper_before_the_passphrase() {
    let source = fixtures::braided(4);
    let remote = match SshRemote::of(&source, HostKey::Unknown) {
        Ok(remote) => remote,
        Err(why) => {
            return skipped(
                "an_unknown_host_key_is_confirmed_through_the_helper_before_the_passphrase",
                &why,
            );
        }
    };
    let serving = Serving::serving(answers_with(String::new(), remote.passphrase().to_owned()));
    let local = with_origin(remote.url());
    local.git(&["config", "core.sshCommand", &remote.ssh_command()]);

    let (outcome, _) = fetch_origin(&serving, &local, remote.home(), None);
    outcome.unwrap_or_else(|e| panic!("the fetch failed: {e}"));

    let prompts = serving.prompts();
    assert_eq!(prompts.len(), 2, "{prompts:?}");
    assert_eq!(
        PromptKind::of(&prompts[0]),
        PromptKind::Confirmation,
        "{:?}",
        prompts[0]
    );
    assert!(
        prompts[0].contains("fingerprint"),
        "the confirmation does not show the fingerprint: {:?}",
        prompts[0]
    );
    assert_eq!(prompts[1], remote.passphrase_prompt());
    assert_eq!(fetched_main(&local), head_of(&source));
}

/// PRD B4 over ssh: an agent holding the key means no prompt at all.
#[test]
fn an_agent_that_holds_the_key_means_no_prompt_at_all() {
    let source = fixtures::braided(4);
    let remote = match SshRemote::of(&source, HostKey::Known) {
        Ok(remote) => remote,
        Err(why) => return skipped("an_agent_that_holds_the_key_means_no_prompt_at_all", &why),
    };
    let agent = match remote.agent_holding_the_key() {
        Ok(agent) => agent,
        Err(why) => return skipped("an_agent_that_holds_the_key_means_no_prompt_at_all", &why),
    };
    let serving = Serving::serving(refusing());
    let local = with_origin(remote.url());
    local.git(&["config", "core.sshCommand", &remote.ssh_command()]);

    let (outcome, _) = fetch_origin(&serving, &local, remote.home(), Some(agent.socket()));
    outcome.unwrap_or_else(|e| panic!("the fetch failed: {e}"));

    assert_eq!(
        serving.prompts(),
        Vec::<String>::new(),
        "Cairn prompted even though the user's agent holds the key (L7, PRD B4)"
    );
    assert_eq!(fetched_main(&local), head_of(&source));
}

/// PRD B5 over ssh: refusing the passphrase fails the fetch cleanly.
#[test]
fn refusing_the_passphrase_fails_the_ssh_fetch_cleanly() {
    let source = fixtures::braided(4);
    let remote = match SshRemote::of(&source, HostKey::Known) {
        Ok(remote) => remote,
        Err(why) => return skipped("refusing_the_passphrase_fails_the_ssh_fetch_cleanly", &why),
    };
    let serving = Serving::serving(refusing());
    let local = with_origin(remote.url());
    local.git(&["config", "core.sshCommand", &remote.ssh_command()]);

    let started = Instant::now();
    let (outcome, _) = fetch_origin(&serving, &local, remote.home(), None);
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "a refusal took {:?}",
        started.elapsed()
    );
    let error = match outcome {
        Err(error) => error,
        Ok(performed) => panic!("a refused passphrase fetched anyway: {performed:?}"),
    };
    assert!(matches!(error, Error::GitFailed { .. }), "{error:?}");
    no_secret_in(
        &error.to_string(),
        remote.passphrase(),
        "the error shown to the user",
    );
    assert_eq!(serving.prompts(), [remote.passphrase_prompt()]);
    let left =
        askpass::wait_for_no_process_pointed_at(serving.socket_path(), Duration::from_secs(5));
    assert!(
        left.is_empty(),
        "processes left behind after the refusal: {left:?}"
    );
}
