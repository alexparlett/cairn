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
use std::path::Path;
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
    assert!(
        cancelled_at.elapsed() < Duration::from_secs(5),
        "the cancel took {:?} to end the wait",
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

/// Issue #17: `ops::fetch` takes no `Confirmed` on the claim that it deletes
/// nothing, and `--no-prune --no-prune-tags` is what keeps that true under a
/// user's `fetch.prune` / `fetch.pruneTags` (and their `remote.<name>.*`
/// forms). A stale remote-tracking ref and a purely local tag survive Cairn's
/// fetch; the control then runs plain `git fetch` under the same configuration
/// and both go — so the refs survived because of the arguments, not because
/// the configuration was not being read. The remote is a local path: nothing
/// here is about credentials, and the channel refuses anything asked.
#[test]
fn a_fetch_never_prunes_however_the_repository_is_configured() {
    let source = fixtures::braided(4);
    let serving = Serving::serving(refusing());
    let local = with_origin(&source.path().display().to_string());
    local.git(&["fetch", "--quiet", "origin"]);
    let tip = fetched_main(&local);
    local.git(&["update-ref", "refs/remotes/origin/gone", &tip]);
    local.git(&["tag", "local-only", &tip]);
    for setting in [
        "fetch.prune",
        "fetch.pruneTags",
        "remote.origin.prune",
        "remote.origin.pruneTags",
    ] {
        local.git(&["config", setting, "true"]);
    }
    let stale = || {
        local.git(&[
            "for-each-ref",
            "--format=%(refname)",
            "refs/remotes/origin/gone",
            "refs/tags/local-only",
        ])
    };
    assert_eq!(stale(), "refs/remotes/origin/gone\nrefs/tags/local-only\n");

    let (outcome, _) = fetch_origin(&serving, &local, local.path(), None);
    outcome.unwrap_or_else(|e| panic!("the fetch failed: {e}"));
    assert_eq!(
        stale(),
        "refs/remotes/origin/gone\nrefs/tags/local-only\n",
        "Cairn's fetch deleted a ref under the user's prune configuration"
    );
    assert_eq!(serving.prompts(), Vec::<String>::new());

    // The control: plain git, same repository, same configuration.
    local.git(&["fetch", "--quiet", "origin"]);
    assert_eq!(
        stale(),
        "",
        "plain git did not prune here, so the assertion above decided nothing"
    );
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
