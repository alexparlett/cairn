//! Whether `git` would open a repository it found by searching: `safe.bareRepository`.
//!
//! Cairn opens a repository the way `git` finds one — from a directory, walking upwards —
//! and then names it to every `git` it runs with `--git-dir` (`process/cli.rs`), which is
//! the explicit spelling git never refuses. So the refusal git applies to a bare
//! repository found by searching has to be applied here, when the repository is opened, or
//! a repository the user's own `git` refuses is read anyway — and with it whatever its
//! configuration names: `core.fsmonitor` runs on every read, `core.sshCommand` and
//! `credential.helper` on a fetch. That is the attack the setting exists for: a bare
//! repository planted inside a cloned working tree, found by whoever opens that directory.
//!
//! The rule is `setup_git_directory_gently_1` in git's `setup.c`, read at v2.38.0, v2.43.0,
//! v2.44.0, v2.44.1, v2.45.0 and v2.56.0 and reproduced against 2.30.9, 2.32.7, 2.39.5,
//! 2.40.0 and 2.56.0. git's search tries each directory from the starting one upwards: a
//! `.git` in it (a file naming the git directory, or a git directory) is a working tree,
//! and the directory itself being a git directory is a bare repository found by
//! searching. Only that second case is checked, and only when the configuration git
//! protects says `explicit`; then the repository is refused unless git calls it
//! "implicit", which depends on the version of the `git` asked:
//!
//! - before 2.38, there is no setting, and nothing is refused;
//! - from 2.38 to 2.43, every bare repository found by searching is refused, a `.git`
//!   directory entered directly included;
//! - in 2.44, a directory named `.git` is allowed;
//! - from 2.45, so is one whose path holds `/.git/worktrees/` (a linked worktree's git
//!   directory) or `/.git/modules/` (a submodule's).
//!
//! The setting is read only from the configuration git protects (`git_protected_config`):
//! the system file, the global ones and the command line's (`GIT_CONFIG_COUNT` and
//! `GIT_CONFIG_PARAMETERS`, which `git -c` sets), never the repository's own — the file an
//! attacker writes. Includes are followed — except by 2.38.x, whose `read_protected_config`
//! added each file without them, and named the system file without asking whether
//! `GIT_CONFIG_NOSYSTEM` wanted it ([`Protected`]) — but not `includeIf "gitdir:"`, which git
//! evaluates there with no repository. The same reader serves `safe.directory`
//! (`crate::ownership`), through [`protected_values`]. It is read whenever the search stops at a bare
//! repository, implicit or not, and every value is checked as git checks it: `explicit` or
//! `all` exactly, and anything else — another case, the bare key, an empty value — is a
//! value git dies on, wherever it sits; the last value wins. The default is `all` before
//! git 3.0 and `explicit` from it (`Documentation/BreakingChanges.adoc`).
//!
//! The environment those files and variables are found through is the one Cairn was
//! launched with: what the user's own `git`, run from the same place, reads. Residual
//! review obligations, stated rather than implied: the system file is `GIT_CONFIG_SYSTEM`
//! or `/etc/gitconfig`, not the path compiled into the `git` Cairn found (the same file in
//! a distribution's git, whose prefix is `/usr`; another file in a git built with another
//! `sysconfdir`, which asking would mean running a process); an `includeIf "hasconfig:"`
//! in a global file, which git can match there, is not followed; a command line git cannot
//! parse is refused only where the search stops at a bare repository, while git refuses
//! every command with it; and a git built `WITH_BREAKING_CHANGES` before 3.0 defaults to
//! `explicit` where this reads `all`.
//!
//! The search that is checked is the search that opens: [`find`] hands back where it
//! stopped, and `SharedRepository` decides ownership over exactly that
//! (`crate::ownership`) and opens exactly that, never searching again.

use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::OsStrExt as _;
use std::path::{Path, PathBuf};

use crate::Error;
use crate::ops::GitVersion;

const fn version(major: u32, minor: u32) -> GitVersion {
    GitVersion {
        major,
        minor,
        patch: 0,
    }
}

/// The first git that reads `safe.bareRepository`.
const SETTING_FROM: GitVersion = version(2, 38);
/// The first git that lets a `.git` directory found by searching open under `explicit`.
const DOT_GIT_FROM: GitVersion = version(2, 44);
/// The first git that lets a worktree's or a submodule's git directory open too.
const WORKTREES_FROM: GitVersion = version(2, 45);
/// The first git whose default is `explicit`.
const EXPLICIT_BY_DEFAULT_FROM: GitVersion = version(3, 0);

/// The setting's key, as git spells it.
const KEY: &str = "safe.bareRepository";

/// Where git's search from `start` stops, with the path to hand gix to open exactly
/// there — the `.git` of a working tree, or a git directory found as itself — physical
/// and absolute, or the refusal git at `version` gives it:
/// [`Error::NotARepository`] when the search finds nothing,
/// [`Error::BareRepositoryFoundBySearching`] for a bare repository the setting refuses,
/// [`Error::InvalidConfig`] for a value git dies on. `environment` answers what the
/// launching environment holds for a name.
///
/// The repository opened is the one this search checked, never a second search's: two
/// walks agree only while they take the same steps, and one that followed a link
/// logically — `docs/guide -> ../guide` searched upwards into `docs/` — would open a bare
/// repository planted there while this one passed the working tree above it.
pub(crate) fn find(
    start: &Path,
    version: GitVersion,
    environment: &dyn Fn(&str) -> Option<OsString>,
) -> Result<Stop, Error> {
    let stop = search(start).ok_or_else(|| Error::NotARepository {
        path: start.to_owned(),
    })?;
    let found = match stop {
        Stop::WorkTree(dot_git) => return Ok(Stop::WorkTree(dot_git)),
        Stop::GitDirectory(found) => found,
    };
    if version < SETTING_FROM {
        return Ok(Stop::GitDirectory(found));
    }
    // Read before the path is looked at, as git reads it (`get_allowed_bare_repo()` is the
    // condition's first operand): a value git dies on refuses an implicit one too.
    let explicit = match protected_setting(environment, version)? {
        Some(Setting::Explicit) => true,
        Some(Setting::All) => false,
        None => version >= EXPLICIT_BY_DEFAULT_FROM,
    };
    if explicit && !is_implicit(&found, version) {
        return Err(Error::BareRepositoryFoundBySearching { path: found });
    }
    Ok(Stop::GitDirectory(found))
}

/// Where git's search stops.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Stop {
    /// A directory holding a `.git` that is a repository: the path is that `.git`.
    WorkTree(PathBuf),
    /// A directory that is itself a git directory: a bare repository found by searching.
    GitDirectory(PathBuf),
}

impl Stop {
    /// The path gix is handed to open: the `.git` of a working tree, or the git directory.
    pub(crate) fn into_path(self) -> PathBuf {
        match self {
            Self::WorkTree(path) | Self::GitDirectory(path) => path,
        }
    }
}

/// Git's search from `start`: from the physical directory upwards (git searches from
/// `getcwd`, which resolves links), the first directory with a `.git` that is a repository
/// is a working tree, and the first that is itself a git directory is a bare repository; a
/// directory named `.git` is checked as itself, which git also ends up doing. Like git, and
/// like the search gix makes by default, it does not cross into another filesystem. `None`
/// when it finds nothing, or `start` is not a directory that can be read.
fn search(start: &Path) -> Option<Stop> {
    use std::os::unix::fs::MetadataExt as _;

    let mut cursor = std::fs::canonicalize(start).ok()?;
    let device = std::fs::metadata(&cursor).ok()?.dev();
    loop {
        if std::fs::metadata(&cursor).ok()?.dev() != device {
            return None;
        }
        let dot_git = cursor.join(".git");
        if cursor.file_name() != Some(OsStr::new(".git")) && gix::discover::is_git(&dot_git).is_ok()
        {
            return Some(Stop::WorkTree(dot_git));
        }
        if gix::discover::is_git(&cursor).is_ok() {
            return Some(Stop::GitDirectory(cursor));
        }
        if !cursor.pop() {
            return None;
        }
    }
}

/// `is_implicit_bare_repo` in git's `setup.c`, as the git at `version` has it. `path` is
/// physical and absolute, as git's is.
fn is_implicit(path: &Path, version: GitVersion) -> bool {
    let named_dot_git = path.file_name() == Some(OsStr::new(".git"));
    let bytes = path.as_os_str().as_bytes();
    let holds = |needle: &[u8]| bytes.windows(needle.len()).any(|window| window == needle);
    if version >= WORKTREES_FROM {
        named_dot_git || holds(b"/.git/worktrees/") || holds(b"/.git/modules/")
    } else if version >= DOT_GIT_FROM {
        named_dot_git
    } else {
        false
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Setting {
    All,
    Explicit,
}

/// `allowed_bare_repo_cb`: `explicit` or `all`, exactly; git dies on anything else.
fn parse(value: Option<&[u8]>) -> Result<Setting, Error> {
    match value {
        Some(b"explicit") => Ok(Setting::Explicit),
        Some(b"all") => Ok(Setting::All),
        other => Err(Error::InvalidConfig {
            key: KEY.to_owned(),
            value: other.map_or_else(
                || "(no value)".to_owned(),
                |value| String::from_utf8_lossy(value).into_owned(),
            ),
        }),
    }
}

/// The setting as the configuration git protects holds it, every value checked and the
/// last one winning; `None` when nothing sets it.
fn protected_setting(
    environment: &dyn Fn(&str) -> Option<OsString>,
    version: GitVersion,
) -> Result<Option<Setting>, Error> {
    let values = protected_values(
        "bareRepository",
        environment,
        Protected {
            includes: follows_includes(version),
            command_line: true,
            file_variables: version >= FILE_VARIABLES_FROM,
            nosystem: honours_nosystem(version),
        },
    )?;
    let mut setting = None;
    for value in values {
        setting = Some(parse(value.as_deref())?);
    }
    Ok(setting)
}

/// How git reads the configuration it protects, which changed with its version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Protected {
    /// Whether `include.path` is followed: by every git but 2.38's, whose
    /// `read_protected_config` added each file with `git_configset_add_file`, which follows
    /// none (2.39 went back to `config_with_options`, which does).
    pub(crate) includes: bool,
    /// Whether the command line's configuration (`git -c`, `GIT_CONFIG_PARAMETERS`,
    /// `GIT_CONFIG_COUNT`) counts: from 2.38, whose `git_protected_config` reads it; the
    /// `read_very_early_config` before it skips it.
    pub(crate) command_line: bool,
    /// Whether `GIT_CONFIG_GLOBAL` and `GIT_CONFIG_SYSTEM` name the files: from 2.32, which
    /// introduced them; a git before it reads `~/.gitconfig`, the XDG file and
    /// `/etc/gitconfig` whatever they say (`GIT_CONFIG_NOSYSTEM` is older, and read by
    /// both).
    pub(crate) file_variables: bool,
    /// Whether `GIT_CONFIG_NOSYSTEM` keeps the system file out: by every git but 2.38's,
    /// whose `read_protected_config` names it with `git_system_config()` and never asks
    /// `git_config_system()` (2.39's `config_with_options` asks again).
    pub(crate) nosystem: bool,
}

/// The first git that reads `GIT_CONFIG_GLOBAL` and `GIT_CONFIG_SYSTEM`.
pub(crate) const FILE_VARIABLES_FROM: GitVersion = version(2, 32);

/// The first git whose protected configuration follows no include.
const INCLUDES_SKIPPED_FROM: GitVersion = version(2, 38);
/// The first git after it that follows them again.
const INCLUDES_FOLLOWED_AGAIN_FROM: GitVersion = version(2, 39);

/// Whether the git at `version` follows `include.path` in the configuration it protects:
/// every git but 2.38.x.
pub(crate) fn follows_includes(version: GitVersion) -> bool {
    !(INCLUDES_SKIPPED_FROM..INCLUDES_FOLLOWED_AGAIN_FROM).contains(&version)
}

/// Whether the git at `version` keeps the system file out of the configuration it protects
/// under `GIT_CONFIG_NOSYSTEM`: every git but 2.38.x, the same reader that follows no
/// include (read in `config.c` at v2.38.0, v2.38.5 and v2.39.0).
pub(crate) fn honours_nosystem(version: GitVersion) -> bool {
    follows_includes(version)
}

/// Every value the configuration git protects gives `safe.<name>`, in git's order — the
/// system file, the XDG file, `~/.gitconfig` (or `GIT_CONFIG_GLOBAL` for both), then the
/// command line when `reading` counts it — every value checked as each reader of it checks
/// it. `None` is a value given as the bare key on the command line; in a file the bare key
/// reads as an empty value, which every reader treats alike.
pub(crate) fn protected_values(
    name: &str,
    environment: &dyn Fn(&str) -> Option<OsString>,
    reading: Protected,
) -> Result<Vec<Option<Vec<u8>>>, Error> {
    let files_environment = |variable: &str| match variable {
        "GIT_CONFIG_GLOBAL" | "GIT_CONFIG_SYSTEM" if !reading.file_variables => None,
        "GIT_CONFIG_NOSYSTEM" if !reading.nosystem => None,
        other => environment(other),
    };
    let mut values = files_values(name, &files_environment, reading.includes)?;
    if reading.command_line {
        values.extend(command_line_values(name, environment)?);
    }
    Ok(values)
}

/// Every value the system and global files give `safe.<name>`, in git's order: system, then
/// the XDG file, then `~/.gitconfig` (or `GIT_CONFIG_GLOBAL` for both), includes followed in
/// place when `includes` says so. A file that cannot be read or parsed is the refusal git
/// gives it.
///
/// The system file is `GIT_CONFIG_SYSTEM`, or `/etc/gitconfig` (unless
/// `GIT_CONFIG_NOSYSTEM`): gix's `Source::System`, which names a path and runs nothing.
/// Not `Source::GitInstallation`, which finds its file by running the `git` on `PATH` —
/// `git config -lz --show-origin` from the process's own environment, outside
/// `GitEnvironment` — and which gix's own open never asks for.
fn files_values(
    name: &str,
    environment: &dyn Fn(&str) -> Option<OsString>,
    includes: bool,
) -> Result<Vec<Option<Vec<u8>>>, Error> {
    use gix::config::{File, Source, file::Metadata, file::includes, file::init};

    let mut lookup = |name: &str| environment(name);
    let metas: Vec<Metadata> = [Source::System, Source::Git, Source::User]
        .into_iter()
        .filter_map(|source| {
            let path = source
                .storage_location(&mut lookup)
                .filter(|path| path.is_file())?;
            Some(Metadata {
                path: Some(path),
                source,
                level: 0,
                trust: gix::sec::Trust::Full,
            })
        })
        .collect();
    let home = environment("HOME").map(PathBuf::from);
    let options = init::Options {
        includes: if includes {
            includes::Options::follow_without_conditional(home.as_deref())
        } else {
            includes::Options::no_follow()
        },
        ..Default::default()
    };
    let file =
        File::from_paths_metadata(metas, options).map_err(|source| Error::ProtectedConfig {
            source: Box::new(source),
        })?;
    let mut values = Vec::new();
    let Some(file) = file else {
        return Ok(values);
    };
    for section in file.sections_by_name("safe").into_iter().flatten() {
        if section.header().subsection_name().is_some() {
            continue;
        }
        for (key, value) in section.body() {
            if key.eq_ignore_ascii_case(name) {
                // A key with no `=` reads as an empty value, which git refuses too.
                values.push(Some(value.to_vec()));
            }
        }
    }
    Ok(values)
}

/// Every value the command line gives `safe.<name>`: `GIT_CONFIG_COUNT`'s pairs, then
/// `GIT_CONFIG_PARAMETERS`, as `git_config_from_parameters` reads them. `None` is a value
/// given as the bare key.
fn command_line_values(
    name: &str,
    environment: &dyn Fn(&str) -> Option<OsString>,
) -> Result<Vec<Option<Vec<u8>>>, Error> {
    let malformed = |variable: &str, value: &OsStr| Error::InvalidConfig {
        key: variable.to_owned(),
        value: value.to_string_lossy().into_owned(),
    };
    let mut values = Vec::new();
    if let Some(count) = environment("GIT_CONFIG_COUNT") {
        let parsed =
            entry_count(count.as_bytes()).map_err(|_| malformed("GIT_CONFIG_COUNT", &count))?;
        for index in 0..parsed {
            let key_name = format!("GIT_CONFIG_KEY_{index}");
            let value_name = format!("GIT_CONFIG_VALUE_{index}");
            let key = environment(&key_name).ok_or_else(|| malformed(&key_name, OsStr::new("")))?;
            let value =
                environment(&value_name).ok_or_else(|| malformed(&value_name, OsStr::new("")))?;
            if is_key(key.as_bytes(), name) {
                values.push(Some(value.as_bytes().to_vec()));
            }
        }
    }
    if let Some(parameters) = environment("GIT_CONFIG_PARAMETERS") {
        let pairs = parameter_pairs(parameters.as_bytes())
            .ok_or_else(|| malformed("GIT_CONFIG_PARAMETERS", &parameters))?;
        for (key, value) in pairs {
            if is_key(&key, name) {
                values.push(value);
            }
        }
    }
    Ok(values)
}

/// Why git refuses a `GIT_CONFIG_COUNT`, in the words of its error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CountRefused {
    /// "bogus count in GIT_CONFIG_COUNT": something other than a number, or after one.
    Bogus,
    /// "too many entries in GIT_CONFIG_COUNT": a number past `INT_MAX`.
    TooMany,
}

/// `GIT_CONFIG_COUNT` as `git_config_from_parameters` reads it (git 2.31 on; before, it is
/// not read, and before 2.38 nothing here is): C's `strtoul(text, &end, 10)`, then "bogus
/// count" when anything is left after the number and "too many entries" past `INT_MAX`.
/// So leading whitespace (C's `isspace`, in the C locale) and one sign are accepted; an
/// empty value is zero entries, but whitespace or a sign alone is bogus, since `strtoul`
/// then consumes nothing; a negative number is negated modulo `ULONG_MAX + 1`, which puts
/// all but the last `INT_MAX` of them past `INT_MAX`; and a number past `ULONG_MAX`,
/// either sign, is `ULONG_MAX`. Each verified against git 2.56; git 2.30.9 ignores the
/// variable.
fn entry_count(text: &[u8]) -> Result<usize, CountRefused> {
    use std::ffi::{c_int, c_ulong};

    let (value, consumed) = strtoul(text);
    // `end` left anywhere but the text's end is bogus; nothing consumed leaves it at the
    // start, so only an empty text is a count there.
    if consumed != text.len() {
        return Err(CountRefused::Bogus);
    }
    // git does not look at `errno`: an overflow is `ULONG_MAX`, past `INT_MAX`.
    let value = value.unwrap_or(c_ulong::MAX);
    if value > c_ulong::from(c_int::MAX.unsigned_abs()) {
        return Err(CountRefused::TooMany);
    }
    usize::try_from(value).map_err(|_| CountRefused::TooMany)
}

/// C's `strtoul(text, &end, 10)` in the C locale: leading whitespace (C's `isspace`), one
/// sign, then decimal digits; a negative number negated modulo `ULONG_MAX + 1`. Answers the
/// value — `None` where it overflowed, which C reports as `ULONG_MAX` with `errno` set to
/// `ERANGE`, either sign — and how many bytes `end` moved past: none at all when no digit
/// follows the whitespace and sign, since C then leaves `end` at the start.
pub(crate) fn strtoul(text: &[u8]) -> (Option<std::ffi::c_ulong>, usize) {
    use std::ffi::c_ulong;

    let mut at = 0;
    while text.get(at).copied().is_some_and(is_space) {
        at += 1;
    }
    let negative = text.get(at) == Some(&b'-');
    if matches!(text.get(at), Some(b'-' | b'+')) {
        at += 1;
    }
    let digits = text[at.min(text.len())..]
        .iter()
        .take_while(|byte| byte.is_ascii_digit())
        .count();
    if digits == 0 {
        return (Some(0), 0);
    }
    let mut value: Option<c_ulong> = Some(0);
    for byte in &text[at..at + digits] {
        value = value
            .and_then(|value| value.checked_mul(10))
            .and_then(|value| value.checked_add(c_ulong::from(byte - b'0')));
    }
    let value = match value {
        Some(value) if negative => Some(value.wrapping_neg()),
        other => other,
    };
    (value, at + digits)
}

/// Whether a command-line key is `safe.<name>`, which git compares without case once it has
/// lower-cased the section and the name (a subsection between them would make it another
/// key).
fn is_key(key: &[u8], name: &str) -> bool {
    key.split_at_checked(b"safe.".len())
        .is_some_and(|(section, rest)| {
            section.eq_ignore_ascii_case(b"safe.") && rest.eq_ignore_ascii_case(name.as_bytes())
        })
}

/// One command-line entry: its key, and its value (`None` for the bare key).
type Pair = (Vec<u8>, Option<Vec<u8>>);

/// `parse_config_env_list`: whitespace-separated entries, each `'key'='value'` (git 2.31
/// and later), `'key'=` for the bare key, or the older `'key=value'` and `'key'`, every
/// part single-quoted as `sq_quote` writes it. `None` for anything git calls bogus, or an
/// empty key. A key git's key parser refuses for other reasons is not looked at: git then
/// refuses every command, whichever repository it is in.
fn parameter_pairs(mut text: &[u8]) -> Option<Vec<Pair>> {
    let mut pairs = Vec::new();
    while !text.is_empty() {
        let (key, rest) = dequote_step(text)?;
        match rest.first().copied() {
            None => {
                old_style(&key, &mut pairs)?;
                text = rest;
            }
            Some(b'=') => {
                let after = &rest[1..];
                match after.first().copied() {
                    Some(b'\'') => {
                        let (value, rest) = dequote_step(after)?;
                        if key.is_empty() || rest.first().is_some_and(|byte| !is_space(*byte)) {
                            return None;
                        }
                        pairs.push((key, Some(value)));
                        text = rest;
                    }
                    Some(byte) if !is_space(byte) => return None,
                    _ if key.is_empty() => return None,
                    _ => {
                        pairs.push((key, None));
                        text = after;
                    }
                }
            }
            Some(byte) if !is_space(byte) => return None,
            Some(_) => {
                old_style(&key, &mut pairs)?;
                text = rest;
            }
        }
        while let [first, tail @ ..] = text
            && is_space(*first)
        {
            text = tail;
        }
    }
    Some(pairs)
}

/// An old-style entry: the key and its value in one quoted string, split at the first
/// `=`; no `=` is the bare key, and an empty key is bogus.
fn old_style(entry: &[u8], pairs: &mut Vec<Pair>) -> Option<()> {
    let mut parts = entry.splitn(2, |byte| *byte == b'=');
    let name = parts.next().unwrap_or_default().to_vec();
    if name.is_empty() {
        return None;
    }
    pairs.push((name, parts.next().map(<[u8]>::to_vec)));
    Some(())
}

/// C's `isspace` in the C locale, which git's parser uses.
pub(crate) fn is_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c)
}

/// `sq_dequote_step`: one single-quoted word, where `'\''` and `'\!'` stand for a quote
/// and a `!`, and what follows it. `None` when `text` does not start with a quote or the
/// quote is never closed.
fn dequote_step(text: &[u8]) -> Option<(Vec<u8>, &[u8])> {
    let mut rest = text.strip_prefix(b"'")?;
    let mut word = Vec::new();
    loop {
        let (&byte, tail) = rest.split_first()?;
        rest = tail;
        if byte != b'\'' {
            word.push(byte);
            continue;
        }
        match rest {
            [b'\\', escaped @ (b'\'' | b'!'), b'\'', tail @ ..] => {
                word.push(*escaped);
                rest = tail;
            }
            _ => return Some((word, rest)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(major: u32, minor: u32, patch: u32) -> GitVersion {
        GitVersion {
            major,
            minor,
            patch,
        }
    }

    /// Each band of git's rule, at its edges. Caught by: a band moved by a version, a
    /// `.git` directory allowed before 2.44, or a worktree's allowed before 2.45.
    #[test]
    fn which_bare_repositories_are_implicit_follows_the_version_of_git() {
        let dot_git = Path::new("/home/u/repo/.git");
        let worktree = Path::new("/home/u/repo/.git/worktrees/feature");
        let module = Path::new("/home/u/repo/.git/modules/sub");
        let planted = Path::new("/home/u/repo/evil.git");
        let lookalike = Path::new("/home/u/repo/x.git/worktrees/feature");
        let cases: [(GitVersion, [bool; 5]); 6] = [
            (at(2, 38, 0), [false, false, false, false, false]),
            (at(2, 43, 5), [false, false, false, false, false]),
            (at(2, 44, 0), [true, false, false, false, false]),
            (at(2, 44, 1), [true, false, false, false, false]),
            (at(2, 45, 0), [true, true, true, false, false]),
            (at(2, 56, 0), [true, true, true, false, false]),
        ];
        for (version, expected) in cases {
            let found = [dot_git, worktree, module, planted, lookalike]
                .map(|path| is_implicit(path, version));
            assert_eq!(found, expected, "git {version:?}");
        }
    }

    /// A git before 2.38 has nothing to refuse, so nothing is read: even a configuration
    /// git would die on opens, from this checkout's git directory entered directly, which
    /// a git with the setting reads it for and dies on. Caught by: checking on a git with
    /// no setting.
    #[test]
    fn a_git_without_the_setting_refuses_nothing() {
        let environment = |name: &str| match name {
            "GIT_CONFIG_PARAMETERS" => Some(OsString::from("'safe.bareRepository'='bogus'")),
            "GIT_CONFIG_NOSYSTEM" => Some(OsString::from("1")),
            "GIT_CONFIG_GLOBAL" => Some(OsString::from("/dev/null")),
            _ => None,
        };
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let git_dir = match search(root) {
            Some(Stop::WorkTree(dot_git)) => {
                gix::discover::path::from_gitdir_file(&dot_git).unwrap_or_else(|_| dot_git.clone())
            }
            _ => panic!("this checkout is not a working tree"),
        };
        assert!(matches!(search(&git_dir), Some(Stop::GitDirectory(_))));
        assert!(find(&git_dir, at(2, 37, 7), &environment).is_ok());
        assert!(matches!(
            find(&git_dir, at(2, 38, 0), &environment),
            Err(Error::InvalidConfig { .. })
        ));
    }

    /// git's two values exactly, and a death on anything else. Caught by: a value read
    /// without case, as a boolean, or an unknown one ignored.
    #[test]
    fn only_explicit_and_all_are_values() {
        assert_eq!(parse(Some(b"explicit")).unwrap(), Setting::Explicit);
        assert_eq!(parse(Some(b"all")).unwrap(), Setting::All);
        for refused in [Some(&b"Explicit"[..]), Some(b""), Some(b"true"), None] {
            assert!(
                matches!(parse(refused), Err(Error::InvalidConfig { .. })),
                "{refused:?} was accepted"
            );
        }
    }

    /// `GIT_CONFIG_PARAMETERS` as git writes and reads it, new style and old. Caught by:
    /// an escaped quote read as the end of a word, the bare key read as a value, or a
    /// bogus entry accepted.
    #[test]
    fn command_line_parameters_are_read_as_git_reads_them() {
        let read = |text: &str| parameter_pairs(text.as_bytes());
        let pair = |key: &str, value: Option<&str>| {
            (
                key.as_bytes().to_vec(),
                value.map(|value| value.as_bytes().to_vec()),
            )
        };
        assert_eq!(
            read("'safe.bareRepository'='explicit' 'a.b'='it'\\''s'  'c.d'="),
            Some(vec![
                pair("safe.bareRepository", Some("explicit")),
                pair("a.b", Some("it's")),
                pair("c.d", None),
            ])
        );
        assert_eq!(
            read("'safe.bareRepository=all' 'e.f'"),
            Some(vec![
                pair("safe.bareRepository", Some("all")),
                pair("e.f", None)
            ])
        );
        assert_eq!(read(""), Some(Vec::new()));
        for bogus in [
            "safe.x=y",
            "'a.b'=c",
            "'a.b",
            "'a.b'='c'd",
            "'=x'",
            "''='x'",
            " 'a.b'",
        ] {
            assert_eq!(read(bogus), None, "{bogus:?} was accepted");
        }
    }

    /// `GIT_CONFIG_COUNT` as git 2.56's `strtoul` reads it, each spelling's answer the one
    /// git gave for it: accepted with leading whitespace and one sign, an empty value as
    /// zero, a negative negated modulo 2^64 (on a 64-bit `long`), an overflow saturated,
    /// and "bogus count" for anything left over and "too many entries" past `INT_MAX` —
    /// the two errors told apart as git tells them. Caught by: `str::parse::<usize>`, which
    /// refuses the whitespace, the empty value and the negatives git accepts, and takes a
    /// count past `INT_MAX` as one.
    #[test]
    fn the_entry_count_is_read_as_gits_strtoul_reads_it() {
        use CountRefused::{Bogus, TooMany};
        let cases: &[(&[u8], Result<usize, CountRefused>)] = &[
            (b"", Ok(0)),
            (b"0", Ok(0)),
            (b"1", Ok(1)),
            (b"01", Ok(1)),
            (b" 1", Ok(1)),
            (b"\t1", Ok(1)),
            (b"\n1", Ok(1)),
            (b"\x0b1", Ok(1)),
            (b"\r\x0c1", Ok(1)),
            (b"+1", Ok(1)),
            (b" +1", Ok(1)),
            (b"-0", Ok(0)),
            (b"2147483647", Ok(2_147_483_647)),
            (b"1x", Err(Bogus)),
            (b"1 ", Err(Bogus)),
            (b"1\t", Err(Bogus)),
            (b"0x1", Err(Bogus)),
            (b"  ", Err(Bogus)),
            (b"+", Err(Bogus)),
            (b"-", Err(Bogus)),
            (b"- 1", Err(Bogus)),
            (b"+-1", Err(Bogus)),
            (b"\xc2\xa01", Err(Bogus)),
            (b"99999999999999999999999x", Err(Bogus)),
            (b"-1", Err(TooMany)),
            (b"2147483648", Err(TooMany)),
            (b"4294967296", Err(TooMany)),
            (b"99999999999999999999999", Err(TooMany)),
            (b"-18446744073709551616", Err(TooMany)),
            (b"-18446744071562067968", Err(TooMany)),
        ];
        for (text, expected) in cases {
            let spelled = String::from_utf8_lossy(text);
            assert_eq!(entry_count(text), *expected, "GIT_CONFIG_COUNT={spelled:?}");
        }
        // Where `long` is 64 bits, as on every target Cairn builds for: a negative within
        // `INT_MAX` of 2^64 wraps to a count git accepts.
        if std::ffi::c_ulong::BITS == 64 {
            for (text, expected) in [
                (&b"18446744073709551615"[..], Err(TooMany)),
                (b"-18446744073709551615", Ok(1)),
                (b"-18446744073709551614", Ok(2)),
                (b"-18446744071562067969", Ok(2_147_483_647)),
            ] {
                let spelled = String::from_utf8_lossy(text);
                assert_eq!(entry_count(text), expected, "GIT_CONFIG_COUNT={spelled:?}");
            }
        }
    }

    /// The protected configuration as each git reads it: an include followed by every git
    /// but 2.38's, the command line counted only where the reader counts it, and
    /// `GIT_CONFIG_GLOBAL` honoured only by a git that knows it (2.32 on), the home's
    /// `.gitconfig` read instead before. Caught by: 2.38 following includes (its
    /// `read_protected_config` added each file without them), or a variable read by a git
    /// that predates it.
    #[test]
    fn the_protected_configuration_is_read_as_each_git_reads_it() {
        let scratch = std::env::temp_dir().join(format!(
            "cairn-protected-{}-{}",
            std::process::id(),
            line!()
        ));
        let _ = std::fs::remove_dir_all(&scratch);
        std::fs::create_dir_all(&scratch).unwrap();
        let included = scratch.join("included");
        std::fs::write(&included, "[safe]\n\tdirectory = /included\n").unwrap();
        let global = scratch.join("global");
        std::fs::write(
            &global,
            format!(
                "[safe]\n\tdirectory = /global\n[include]\n\tpath = {}\n",
                included.display()
            ),
        )
        .unwrap();
        std::fs::write(scratch.join(".gitconfig"), "[safe]\n\tdirectory = /home\n").unwrap();
        let environment = |name: &str| match name {
            "GIT_CONFIG_NOSYSTEM" => Some(OsString::from("1")),
            "GIT_CONFIG_GLOBAL" => Some(global.clone().into_os_string()),
            "HOME" | "XDG_CONFIG_HOME" => Some(scratch.clone().into_os_string()),
            "GIT_CONFIG_PARAMETERS" => Some(OsString::from("'safe.directory=/cli'")),
            _ => None,
        };
        let read = |includes, command_line, file_variables| {
            let values: Vec<String> = protected_values(
                "directory",
                &environment,
                Protected {
                    includes,
                    command_line,
                    file_variables,
                    nosystem: true,
                },
            )
            .unwrap()
            .into_iter()
            .map(|value| String::from_utf8(value.unwrap_or_default()).unwrap())
            .collect();
            values
        };
        assert_eq!(read(true, true, true), ["/global", "/included", "/cli"]);
        assert_eq!(read(false, true, true), ["/global", "/cli"]);
        assert_eq!(read(true, false, true), ["/global", "/included"]);
        assert_eq!(read(true, false, false), ["/home"]);
        assert!(follows_includes(at(2, 37, 7)));
        assert!(!follows_includes(at(2, 38, 0)) && !follows_includes(at(2, 38, 5)));
        assert!(follows_includes(at(2, 39, 0)));
        std::fs::remove_dir_all(&scratch).unwrap();
    }

    /// git 2.38.x's `read_protected_config` names the system file with `git_system_config()`
    /// alone, never asking `git_config_system()`, so it reads that file even under
    /// `GIT_CONFIG_NOSYSTEM`; every git before and after skips it (2.39 went back to
    /// `config_with_options`, which asks). For `safe.directory` and `safe.bareRepository`
    /// alike. Caught by: gix's `Source::System`, which honours the variable on every git.
    #[test]
    fn the_system_file_is_read_under_nosystem_by_git_2_38_alone() {
        let scratch =
            std::env::temp_dir().join(format!("cairn-nosystem-{}-{}", std::process::id(), line!()));
        let _ = std::fs::remove_dir_all(&scratch);
        std::fs::create_dir_all(&scratch).unwrap();
        let system = scratch.join("system");
        std::fs::write(
            &system,
            "[safe]\n\tdirectory = /system\n\tbareRepository = explicit\n",
        )
        .unwrap();
        let environment = |name: &str| match name {
            "GIT_CONFIG_NOSYSTEM" => Some(OsString::from("1")),
            "GIT_CONFIG_SYSTEM" => Some(system.clone().into_os_string()),
            "GIT_CONFIG_GLOBAL" => Some(OsString::from("/dev/null")),
            _ => None,
        };
        let directories = |found: GitVersion| -> Vec<Option<Vec<u8>>> {
            protected_values(
                "directory",
                &environment,
                crate::ownership::Rule::of(found).reading,
            )
            .unwrap()
        };
        for (found, read) in [
            (at(2, 37, 7), false),
            (at(2, 38, 0), true),
            (at(2, 38, 5), true),
            (at(2, 39, 0), false),
            (at(2, 56, 0), false),
        ] {
            let expected: Vec<Option<Vec<u8>>> = if read {
                vec![Some(b"/system".to_vec())]
            } else {
                Vec::new()
            };
            assert_eq!(directories(found), expected, "safe.directory, git {found}");
            assert_eq!(
                protected_setting(&environment, found).unwrap(),
                read.then_some(Setting::Explicit),
                "safe.bareRepository, git {found}"
            );
        }
        std::fs::remove_dir_all(&scratch).unwrap();
    }

    /// The command line's values come after the files' and in git's order between them:
    /// `GIT_CONFIG_COUNT`'s pairs, then `GIT_CONFIG_PARAMETERS`, the last winning, a key
    /// matched without case, and a subsection making another key. Caught by: the two
    /// variables read in the other order, or either ignored.
    #[test]
    fn the_command_line_is_read_last_count_first() {
        let environment = |name: &str| {
            let value = match name {
                "GIT_CONFIG_COUNT" => "2",
                "GIT_CONFIG_KEY_0" => "SAFE.BAREREPOSITORY",
                "GIT_CONFIG_VALUE_0" => "explicit",
                "GIT_CONFIG_KEY_1" => "safe.sub.bareRepository",
                "GIT_CONFIG_VALUE_1" => "bogus",
                "GIT_CONFIG_PARAMETERS" => "'safe.bareRepository'='all'",
                "GIT_CONFIG_NOSYSTEM" => "1",
                "GIT_CONFIG_GLOBAL" => "/dev/null",
                _ => return None,
            };
            Some(OsString::from(value))
        };
        assert_eq!(
            protected_setting(&environment, at(2, 45, 0)).unwrap(),
            Some(Setting::All)
        );
        let without_parameters = |name: &str| match name {
            "GIT_CONFIG_PARAMETERS" => None,
            other => environment(other),
        };
        assert_eq!(
            protected_setting(&without_parameters, at(2, 45, 0)).unwrap(),
            Some(Setting::Explicit)
        );
        let short = |name: &str| match name {
            "GIT_CONFIG_COUNT" => Some(OsString::from("3")),
            other => environment(other),
        };
        assert!(matches!(
            protected_setting(&short, at(2, 45, 0)),
            Err(Error::InvalidConfig { .. })
        ));
    }
}
