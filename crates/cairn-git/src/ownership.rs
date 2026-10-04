//! Whether git would open a repository it found by searching, as far as ownership goes:
//! `ensure_valid_ownership` in git's `setup.c`, and `safe.directory`.
//!
//! git checks ownership when it DISCOVERS a repository, never when one is named to it
//! (`--git-dir`), and Cairn names every repository it opens to the `git` it runs
//! (`process/cli.rs`), so git's own check never runs on Cairn's behalf: it is made here,
//! as the repository is opened, and a repository git's discovery refuses is refused —
//! [`Error::DubiousOwnership`], before anything in it is read or run (the user's decision
//! of 2026-10-04: refuse at open where git would refuse, as a bare repository found by
//! searching is refused, `crate::bare_discovery`). Nothing git opens is refused.
//!
//! The rule depends on the version of the `git` that will be asked, and each band below
//! was read in git's own source at every tag from v2.30.0 to v2.56.0 (`setup.c`,
//! `git-compat-util.h`, `config.c`, `path.c`, `abspath.c`), with the maintenance releases
//! that carried a change named beside it, and reproduced against git 2.56.0, 2.32.7 and
//! 2.30.9 by `a_repository_opens_exactly_where_git_opens_it_whatever_safe_directory_says`:
//!
//! - **No check** before 2.30.3, 2.31.2, 2.32.1, 2.33.2, 2.34.2 and 2.35.2 (CVE-2022-24765
//!   landed in those and in 2.36.0): every repository opens.
//! - **One path**, the working tree's top (the directory holding `.git`) or, bare, the git
//!   directory, owned when its owner (`lstat`) is the effective uid; `safe.directory`
//!   compared byte for byte with that path, the empty value resetting.
//! - **`*`** naming every directory, and `GIT_TEST_ASSUME_DIFFERENT_OWNER` (a boolean as
//!   git reads one; a value git cannot read stops git) taking every path as someone else's:
//!   2.30.4, 2.31.3, 2.32.2, 2.33.3, 2.34.3, 2.35.3, 2.36.0.
//! - **`SUDO_UID`**: when the effective uid is root, a root-owned path is owned and any
//!   other is compared with `SUDO_UID` instead (read as git's `extract_id_from_env` reads
//!   it: `strtoul`, the whole value, no overflow, cut to a `uid_t`); for any other user it
//!   is not read at all: 2.30.5, 2.31.4, 2.32.3, 2.33.4, 2.34.4, 2.35.4, 2.36.2, 2.37.0.
//! - **Every path**: the `.git` file when the working tree reaches its git directory
//!   through one (a linked worktree, a submodule's checkout), the working tree's top, and
//!   the git directory itself — for a `.git` file the directory it names, resolved as
//!   `read_gitfile_gently` resolves it: 2.30.5, 2.31.4, 2.32.3, 2.33.4, 2.34.4, 2.35.4,
//!   2.36.2, 2.37.1.
//! - **`GIT_CONFIG_GLOBAL` and `GIT_CONFIG_SYSTEM`** naming the files git reads from 2.32.0,
//!   which introduced them; before, `~/.gitconfig`, the XDG file and `/etc/gitconfig`.
//! - **`%(prefix)/`** expanded (`interpolate_path`) from 2.34.0; `~/` and `~user/` always.
//! - **The command line** (`git -c`, `GIT_CONFIG_PARAMETERS`, `GIT_CONFIG_COUNT`) read
//!   from 2.38.0, when `read_very_early_config` gave way to `git_protected_config`; and
//!   2.38.x alone follows no `include.path` there and reads the system file even under
//!   `GIT_CONFIG_NOSYSTEM` (`crate::bare_discovery::Protected`).
//! - **`<dir>/*`** naming every path under `<dir>/` — never `<dir>` itself: 2.45.3, 2.46.0.
//! - **Normalised** from 2.46.1: an entry neither absolute nor `.` is ignored (git warns);
//!   the entry is resolved as `real_path` resolves it (links followed, `.` and `..`
//!   taken, the last component allowed to be missing), and one that cannot be is skipped;
//!   `.` is the directory git was run from.
//! - **`:(optional)`** stripped from 2.52.0, where an entry naming a missing path stops
//!   git (it crashes on it); from 2.53.0 such an entry is skipped. Missing is `ENOENT`
//!   alone (`is_missing_file`): any other failure to `stat` the path stops git on both.
//!
//! A value git cannot expand (`~nobody-here/`, `~` with no `HOME`) stops git wherever it
//! sits once the list is consulted, and refuses here as [`Error::InvalidConfig`]. The list
//! is consulted only when some path is not owned, and is read whole, the last word winning.
//!
//! The environment that configuration, `HOME` and `SUDO_UID` are read through is the one
//! Cairn was launched with, what the user's own `git`, run from the same place, reads; the
//! effective uid is the process's own ([`Identity::of_this_process`]), read from
//! `/proc/self/status` or, where that cannot be read, as the owner of a file the process
//! creates. git's `geteuid` cannot fail, but those reads can: where neither answers,
//! ownership is not decided — the repository opens where `safe.directory` names it, as
//! git opens it whoever owns it, and is otherwise refused as
//! [`Error::CurrentUserUnknown`], never as dubious ownership. Residual review
//! obligations, stated rather than implied: `%(prefix)/` is expanded against the
//! directory above the `bin/` holding the `git` Cairn found (its links resolved), which is
//! git's compiled-in prefix for an installed git, and is not expanded at all when no `git`
//! is known ([`crate::SharedRepository::discover`]), so such an entry names nothing there;
//! the configuration files are found as `crate::bare_discovery` finds them, with its
//! residuals; the created file's owner is the effective uid on a filesystem that records
//! the creator, which one mounted with a fixed owner (FAT, a squashing NFS) does not, and
//! a temporary directory on one is the review's; and gix, once Cairn has decided git opens
//! a repository, checks the working tree's owner again by its own rule
//! (`gix::sec::identity::is_path_owned_by_current_user` over the directory `core.worktree`
//! names, and gix's `safe.directory` reading, which knows neither the command line, `.`,
//! nor git's normalisation) and, where that rule
//! refuses what git's admits, lowers the repository's trust to reduced — which no open
//! option prevents. That rule decides nothing: the repository's configuration was loaded
//! at full trust and is read whole, the allocation limit gix gives reduced trust is
//! switched off (`crate::repository`), and every `git` Cairn runs in it is named the
//! repository (`process/cli.rs`), since this check is git's own; pinned end to end by
//! `a_repository_cairn_admits_is_read_as_git_reads_it_whatever_gix_makes_of_its_owner`.
//!
//! The real case needs a second owner, so it is a privileged run rather than a gate step:
//! `a_linked_worktree_whose_git_dir_is_someone_elses_is_refused_as_git_refuses_it`, an
//! `#[ignore]`d test in `crates/cairn-git/tests/diff/ownership.rs`, run as root by whoever
//! reviews a change here. Every other case is decided without one: by
//! `GIT_TEST_ASSUME_DIFFERENT_OWNER` against git itself, and by an [`Identity`] whose
//! effective uid is not the owner's in `SharedRepository`'s own tests.

use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::OsStrExt as _;
use std::path::{Path, PathBuf};

use crate::Error;
use crate::bare_discovery::{Protected, Stop};
use crate::ops::GitVersion;

/// The setting's key, as git spells it in its messages.
const KEY: &str = "safe.directory";
/// The variable git's test suite sets to take every path as someone else's.
const ASSUME_DIFFERENT_OWNER: &str = "GIT_TEST_ASSUME_DIFFERENT_OWNER";
/// `ROOT_UID` in `git-compat-util.h`, everywhere Cairn builds.
const ROOT_UID: u32 = 0;
/// `MAXSYMLINKS` as `abspath.c` defines it when the platform does not.
const MAX_SYMLINKS: usize = 32;

const fn version(major: u32, minor: u32, patch: u32) -> GitVersion {
    GitVersion {
        major,
        minor,
        patch,
    }
}

/// Whether `found` carries a change git made in `from` and backported to the maintenance
/// releases `backports` names: `(minor, first patch carrying it)` of a 2.x line.
fn carries(found: GitVersion, from: GitVersion, backports: &[(u32, u32)]) -> bool {
    found >= from
        || (found.major == 2
            && backports
                .iter()
                .any(|&(minor, patch)| found.minor == minor && found.patch >= patch))
}

/// What `:(optional)` before a `safe.directory` entry does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Optional {
    /// Nothing: the prefix is part of a relative entry, which never matches.
    Literal,
    /// Stripped, and an entry naming a missing path stops git (2.52).
    Fatal,
    /// Stripped, and an entry naming a missing path is skipped (2.53 on).
    Skipped,
}

/// git's ownership rule, as the git at one version has it (the bands in the module
/// documentation).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Rule {
    checks: bool,
    test_variable: bool,
    sudo: bool,
    every_path: bool,
    star: bool,
    prefix: bool,
    leading_path: bool,
    normalised: bool,
    optional: Optional,
    pub(crate) reading: Protected,
}

impl Rule {
    pub(crate) fn of(found: GitVersion) -> Self {
        let checks = carries(
            found,
            version(2, 36, 0),
            &[(30, 3), (31, 2), (32, 1), (33, 2), (34, 2), (35, 2)],
        );
        let star = carries(
            found,
            version(2, 36, 0),
            &[(30, 4), (31, 3), (32, 2), (33, 3), (34, 3), (35, 3)],
        );
        let sudo = carries(
            found,
            version(2, 37, 0),
            &[
                (30, 5),
                (31, 4),
                (32, 3),
                (33, 4),
                (34, 4),
                (35, 4),
                (36, 2),
            ],
        );
        let every_path = carries(
            found,
            version(2, 38, 0),
            &[
                (30, 5),
                (31, 4),
                (32, 3),
                (33, 4),
                (34, 4),
                (35, 4),
                (36, 2),
                (37, 1),
            ],
        );
        let optional = if found >= version(2, 53, 0) {
            Optional::Skipped
        } else if found >= version(2, 52, 0) {
            Optional::Fatal
        } else {
            Optional::Literal
        };
        Self {
            checks,
            test_variable: star,
            sudo,
            every_path,
            star,
            prefix: found >= version(2, 34, 0),
            leading_path: carries(found, version(2, 46, 0), &[(45, 3)]),
            normalised: found >= version(2, 46, 1),
            optional,
            reading: Protected {
                includes: crate::bare_discovery::follows_includes(found),
                command_line: found >= version(2, 38, 0),
                file_variables: found >= crate::bare_discovery::FILE_VARIABLES_FROM,
                nosystem: crate::bare_discovery::honours_nosystem(found),
            },
        }
    }
}

/// Who the current user is, and who owns a path: the process's own answers in shipping
/// code ([`Identity::of_this_process`]), another's in a test.
pub(crate) struct Identity<'a> {
    /// The effective uid, as `geteuid` answers it; `None` when it could not be read, and
    /// then ownership is not decided: the repository opens only where `safe.directory`
    /// names it, and is otherwise [`Error::CurrentUserUnknown`].
    pub(crate) euid: Option<u32>,
    /// The owner of a path, by `lstat` (a link is its own owner's); `None` when it cannot
    /// be read, and then the path is not owned, as git's check treats it.
    pub(crate) owner_of: &'a dyn Fn(&Path) -> Option<u32>,
}

impl Identity<'static> {
    pub(crate) fn of_this_process() -> Self {
        Self {
            euid: effective_uid(),
            owner_of: &owner_by_lstat,
        }
    }
}

/// The owner of `path`, not following a final link.
fn owner_by_lstat(path: &Path) -> Option<u32> {
    use std::os::unix::fs::MetadataExt as _;
    std::fs::symlink_metadata(path).ok().map(|meta| meta.uid())
}

/// The process's effective uid without `unsafe` or a new dependency, by the first route
/// that answers ([`effective_uid_by`]): on Linux the second field of `Uid:` in
/// `/proc/self/status` (real, effective, saved, filesystem), which the kernel fills from the
/// credentials themselves — unlike the owner of `/proc/self`, which is root for a process
/// that is not dumpable; and where there is no such file (macOS, or a Linux without `/proc`
/// mounted), the owner of a file this process creates. `None` only when neither answers.
fn effective_uid() -> Option<u32> {
    effective_uid_by(
        &|| std::fs::read_to_string("/proc/self/status").ok(),
        &owner_of_a_created_file,
    )
}

/// [`effective_uid`] over its two routes: the effective uid in `status`'s answer (the text
/// of `/proc/self/status`), or, when that is missing or holds none, `created`'s.
fn effective_uid_by(
    status: &dyn Fn() -> Option<String>,
    created: &dyn Fn() -> Option<u32>,
) -> Option<u32> {
    let from_status = status().and_then(|text| {
        text.lines()
            .find_map(|line| line.strip_prefix("Uid:"))?
            .split_whitespace()
            .nth(1)?
            .parse()
            .ok()
    });
    from_status.or_else(created)
}

/// The owner of a file this process creates in the temporary directory, which the kernel
/// gives the creator's effective uid (on Linux its filesystem uid, the effective one unless
/// `setfsuid` moved it; BSD semantics on macOS); it is removed at once.
fn owner_of_a_created_file() -> Option<u32> {
    use std::os::unix::fs::MetadataExt as _;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos());
    let probe = std::env::temp_dir().join(format!("cairn-euid-{}-{nanos}", std::process::id()));
    let file = std::fs::File::create_new(&probe).ok()?;
    let owner = file.metadata().ok().map(|meta| meta.uid());
    drop(file);
    let _ = std::fs::remove_file(&probe);
    owner
}

/// `is_path_owned_by_current_uid` in `git-compat-util.h`, for a path whose owner is
/// `owner`: the effective uid owns it; when that uid is root and `rule` reads `SUDO_UID`,
/// so does root for a root-owned path, and otherwise `SUDO_UID` stands in for the
/// effective uid — for root alone, never for any other user.
fn owns(rule: &Rule, owner: Option<u32>, euid: Option<u32>, sudo_uid: Option<&OsStr>) -> bool {
    let (Some(owner), Some(mut euid)) = (owner, euid) else {
        return false;
    };
    if rule.sudo && euid == ROOT_UID {
        if owner == ROOT_UID {
            return true;
        }
        if let Some(id) = sudo_uid.and_then(|value| uid_from(value.as_bytes())) {
            euid = id;
        }
    }
    owner == euid
}

/// `extract_id_from_env`: a non-empty value `strtoul` reads whole without overflowing,
/// assigned to a `uid_t` (32 bits, cut as C's assignment cuts it); anything else leaves
/// the effective uid as it was.
fn uid_from(text: &[u8]) -> Option<u32> {
    if text.is_empty() {
        return None;
    }
    let (value, consumed) = crate::bare_discovery::strtoul(text);
    if consumed != text.len() {
        return None;
    }
    let low = value? & std::ffi::c_ulong::from(u32::MAX);
    u32::try_from(low).ok()
}

/// `git_env_bool`: unset is `false`; otherwise `git_parse_maybe_bool` — the empty value,
/// `false`, `no`, `off` and `true`, `yes`, `on` without case, or an `int` as
/// `git_parse_int` reads one, true when not zero — and `None` for anything else, a value
/// git stops on.
pub(crate) fn env_bool(value: Option<&OsStr>) -> Option<bool> {
    let Some(value) = value else {
        return Some(false);
    };
    let text = value.as_bytes();
    if text.is_empty() {
        return Some(false);
    }
    let is = |word: &[u8]| text.eq_ignore_ascii_case(word);
    if is(b"true") || is(b"yes") || is(b"on") {
        return Some(true);
    }
    if is(b"false") || is(b"no") || is(b"off") {
        return Some(false);
    }
    parse_int(text).map(|number| number != 0)
}

/// `git_parse_int`: C's `strtoimax(text, &end, 0)` — whitespace, one sign, then `0x`
/// hexadecimal, `0` octal or decimal — then a unit (`k`, `m` or `g`, without case, or
/// nothing) that must be all that is left, the product within an `int`. `None` where git's
/// parser fails: no digits, an overflow, another unit, or out of range.
fn parse_int(text: &[u8]) -> Option<i64> {
    let mut at = 0;
    while text
        .get(at)
        .copied()
        .is_some_and(crate::bare_discovery::is_space)
    {
        at += 1;
    }
    let negative = text.get(at) == Some(&b'-');
    if matches!(text.get(at), Some(b'-' | b'+')) {
        at += 1;
    }
    let hex_digit_follows = text.get(at + 2).is_some_and(u8::is_ascii_hexdigit);
    let (radix, start) = match (text.get(at), text.get(at + 1)) {
        (Some(b'0'), Some(b'x' | b'X')) if hex_digit_follows => (16, at + 2),
        (Some(b'0'), _) => (8, at),
        _ => (10, at),
    };
    let digits = text[start.min(text.len())..]
        .iter()
        .take_while(|byte| char::from(**byte).is_digit(radix))
        .count();
    if digits == 0 {
        return None;
    }
    let mut magnitude: i128 = 0;
    for byte in &text[start..start + digits] {
        let digit = char::from(*byte).to_digit(radix)?;
        magnitude = magnitude * i128::from(radix) + i128::from(digit);
        if magnitude > i128::from(i64::MAX) + 1 {
            return None;
        }
    }
    let value = if negative { -magnitude } else { magnitude };
    let value = i64::try_from(value).ok()?;
    let unit = &text[start + digits..];
    let factor: i64 = if unit.is_empty() {
        1
    } else if unit.eq_ignore_ascii_case(b"k") {
        1024
    } else if unit.eq_ignore_ascii_case(b"m") {
        1024 * 1024
    } else if unit.eq_ignore_ascii_case(b"g") {
        1024 * 1024 * 1024
    } else {
        return None;
    };
    let max = i64::from(i32::MAX);
    if (value < 0 && (-max - 1) / factor > value) || (value > 0 && max / factor < value) {
        return None;
    }
    Some(value * factor)
}

/// Whether the current user owns each path git checks for one repository: `None` where
/// the repository has no such path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Owners {
    /// The `.git` file naming the git directory, when there is one.
    pub(crate) gitfile: Option<bool>,
    /// The top of the working tree, the directory holding `.git`; `None` when bare.
    pub(crate) work_tree: Option<bool>,
    /// The git directory itself.
    pub(crate) git_dir: bool,
}

impl Owners {
    /// Whether git's check passes on ownership alone, before `safe.directory`: every path
    /// it checks owned, or — before git checked every path — the one it checked, the
    /// working tree's top or, bare, the git directory.
    fn pass(self, every_path: bool) -> bool {
        if every_path {
            self.gitfile.unwrap_or(true) && self.work_tree.unwrap_or(true) && self.git_dir
        } else {
            self.work_tree.unwrap_or(self.git_dir)
        }
    }
}

/// The paths git checks for the repository its search stopped at, each answered by
/// `owned`, and the git directory judged: for a working tree, the `.git` file if `.git` is
/// one, the directory holding `.git`, and the git directory (`.git` itself, or the directory
/// the file names, its path resolved as git resolves it); for a bare repository, the git
/// directory alone. `None` when a `.git` file names nothing that is a git directory — git
/// stops on such a file as it reads it, so there is nothing to judge.
pub(crate) fn owners(stop: &Stop, owned: &dyn Fn(&Path) -> bool) -> Option<(Owners, PathBuf)> {
    match stop {
        Stop::GitDirectory(git_dir) => Some((
            Owners {
                gitfile: None,
                work_tree: None,
                git_dir: owned(git_dir),
            },
            git_dir.clone(),
        )),
        Stop::WorkTree(dot_git) => {
            let work_tree = dot_git.parent().map(owned);
            // `read_gitfile_gently` stats `.git`, following a link: a `.git` that is a
            // directory, or a link to one, is the git directory itself.
            if std::fs::metadata(dot_git).is_ok_and(|meta| meta.is_dir()) {
                return Some((
                    Owners {
                        gitfile: None,
                        work_tree,
                        git_dir: owned(dot_git),
                    },
                    dot_git.clone(),
                ));
            }
            let gitfile = Some(owned(dot_git));
            let git_dir = named_git_dir(dot_git)?;
            Some((
                Owners {
                    gitfile,
                    work_tree,
                    git_dir: owned(&git_dir),
                },
                git_dir,
            ))
        }
    }
}

/// The git directory a `.git` file names, resolved to a physical path as git's
/// `read_gitfile_gently` resolves it (`real_path`); `None` when it names nothing that is a
/// git directory — git stops on such a file as it reads it, before any ownership check.
fn named_git_dir(gitfile: &Path) -> Option<PathBuf> {
    let named = gix::discover::path::from_gitdir_file(gitfile).ok()?;
    gix::discover::is_git(&named).ok()?;
    std::fs::canonicalize(named).ok()
}

/// Where git's ownership check is made: the version of the `git` asked and, for
/// `%(prefix)/`, where it is installed.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Asked<'a> {
    pub(crate) version: GitVersion,
    /// The `git` executable Cairn found, links resolved or not; `None` when no `git` is
    /// known, and then `%(prefix)/` expands to nothing.
    pub(crate) executable: Option<&'a Path>,
}

impl Asked<'_> {
    /// git's compiled-in prefix for an installed git: the directory above the `bin/`
    /// holding the executable, its links resolved.
    fn prefix(&self) -> Option<PathBuf> {
        let executable = std::fs::canonicalize(self.executable?).ok()?;
        Some(executable.parent()?.parent()?.to_owned())
    }
}

/// Decides whether the git `asked` opens the repository its search stopped at, `stop`,
/// searching from `start`: `Ok` with the git directory judged — which must be the one then
/// opened — or the refusal git gives: [`Error::DubiousOwnership`] where it refuses for
/// ownership, [`Error::InvalidConfig`] for a value it stops on, [`Error::NotARepository`]
/// for a `.git` file naming nothing. `environment` answers what the launching environment
/// holds for a name, and `identity` who the user is.
pub(crate) fn decide(
    stop: &Stop,
    start: &Path,
    asked: Asked<'_>,
    environment: &dyn Fn(&str) -> Option<OsString>,
    identity: &Identity<'_>,
) -> Result<PathBuf, Error> {
    let rule = Rule::of(asked.version);
    let sudo_uid = environment("SUDO_UID");
    let owned = |path: &Path| {
        owns(
            &rule,
            (identity.owner_of)(path),
            identity.euid,
            sudo_uid.as_deref(),
        )
    };
    let Some((owners, git_dir)) = owners(stop, &owned) else {
        return Err(Error::NotARepository {
            path: start.to_owned(),
        });
    };
    if !rule.checks {
        return Ok(git_dir);
    }
    // Read first, as git's condition reads it first: a value git stops on stops it however
    // the paths are owned.
    let assume_different = if rule.test_variable {
        let value = environment(ASSUME_DIFFERENT_OWNER);
        env_bool(value.as_deref()).ok_or_else(|| Error::InvalidConfig {
            key: ASSUME_DIFFERENT_OWNER.to_owned(),
            value: value
                .as_deref()
                .map(OsStr::to_string_lossy)
                .unwrap_or_default()
                .into_owned(),
        })?
    } else {
        false
    };
    // git's `geteuid` cannot fail; Cairn's reading of it can, and then whether the paths
    // are the user's is not known: never answered as "someone else's".
    let user_unknown = !assume_different && identity.euid.is_none();
    if !assume_different && !user_unknown && owners.pass(rule.every_path) {
        return Ok(git_dir);
    }
    // The path that identifies the repository: its working tree's top, or its git
    // directory when bare — physical, as git's search holds it.
    let identifies = match stop {
        Stop::WorkTree(dot_git) => dot_git.parent().unwrap_or(dot_git),
        Stop::GitDirectory(git_dir) => git_dir.as_path(),
    };
    let place = Place {
        identifies: identifies.as_os_str().as_bytes(),
        cwd: start,
        home: environment("HOME"),
        prefix: if rule.prefix { asked.prefix() } else { None },
    };
    let named = crate::bare_discovery::protected_values("directory", environment, rule.reading)
        .and_then(|values| names_the_repository(&rule, &values, &place));
    match named {
        // Named, git opens it whoever owns it.
        Ok(true) => Ok(git_dir),
        // Owned, git would open it without reading the setting at all — so neither an
        // unnamed repository nor a value git stops on decides it while the user is unknown.
        _ if user_unknown => Err(Error::CurrentUserUnknown {
            path: identifies.to_owned(),
        }),
        Ok(false) => Err(Error::DubiousOwnership {
            path: identifies.to_owned(),
        }),
        Err(error) => Err(error),
    }
}

/// What a `safe.directory` entry is matched against and expanded with.
struct Place<'a> {
    /// The path that identifies the repository, physical.
    identifies: &'a [u8],
    /// The directory git was run from, physical: what `.` names.
    cwd: &'a Path,
    /// `HOME`, for `~/`.
    home: Option<OsString>,
    /// git's prefix, for `%(prefix)/`; `None` expands it to nothing.
    prefix: Option<PathBuf>,
}

/// `safe_directory_cb` over every value in order, as the git `rule` describes has it:
/// whether the last word names the repository, or the refusal git stops with.
fn names_the_repository(
    rule: &Rule,
    values: &[Option<Vec<u8>>],
    place: &Place<'_>,
) -> Result<bool, Error> {
    let mut is_safe = false;
    for value in values {
        let value = match value.as_deref() {
            None | Some(b"") => {
                is_safe = false;
                continue;
            }
            Some(b"*") if rule.star => {
                is_safe = true;
                continue;
            }
            Some(value) => value,
        };
        let Some(allowed) = expand(rule, value, place)? else {
            continue;
        };
        let check = if rule.normalised {
            if !allowed.starts_with(b"/") && allowed != b"." {
                // git warns "safe.directory '<entry>' not absolute" and goes on.
                continue;
            }
            match real_path(&allowed, place.cwd) {
                Some(normalised) => normalised,
                None => continue,
            }
        } else {
            allowed
        };
        let matched = match check.strip_suffix(b"/*") {
            // `fspathncmp(check, path, len - 1)`: everything under `<dir>/`, not `<dir>`.
            Some(dir) if rule.leading_path => place
                .identifies
                .strip_prefix(dir)
                .is_some_and(|rest| rest.starts_with(b"/")),
            _ => check == place.identifies,
        };
        if matched {
            is_safe = true;
        }
    }
    Ok(is_safe)
}

/// `git_config_pathname` for a `safe.directory` value, as the git `rule` describes has it:
/// `:(optional)` handled, then `interpolate_path` — `%(prefix)/`, `~/`, `~user/` —
/// `Some` path, `None` for an entry git skips, or the refusal for a value git stops on.
fn expand(rule: &Rule, value: &[u8], place: &Place<'_>) -> Result<Option<Vec<u8>>, Error> {
    let stops = || Error::InvalidConfig {
        key: KEY.to_owned(),
        value: String::from_utf8_lossy(value).into_owned(),
    };
    let (optional, path) = match value.strip_prefix(b":(optional)") {
        Some(rest) if rule.optional != Optional::Literal => (true, rest),
        _ => (false, value),
    };
    let expanded = match path.strip_prefix(b"%(prefix)/") {
        Some(rest) if rule.prefix => {
            if rest.starts_with(b"/") {
                rest.to_vec()
            } else {
                let Some(prefix) = place.prefix.as_deref() else {
                    return Ok(None);
                };
                let mut joined = prefix.as_os_str().as_bytes().to_vec();
                joined.push(b'/');
                joined.extend_from_slice(rest);
                joined
            }
        }
        _ => match path.strip_prefix(b"~") {
            Some(after) => {
                let slash = after.iter().position(|byte| *byte == b'/');
                let (user, rest) = after.split_at(slash.unwrap_or(after.len()));
                let home = if user.is_empty() {
                    place.home.clone().ok_or_else(stops)?
                } else {
                    let name = std::str::from_utf8(user).map_err(|_| stops())?;
                    gix::config::path::interpolate::home_for_user(name)
                        .ok_or_else(stops)?
                        .into_os_string()
                };
                let mut joined = home.as_bytes().to_vec();
                joined.extend_from_slice(rest);
                joined
            }
            None => path.to_vec(),
        },
    };
    if optional {
        match std::fs::metadata(OsStr::from_bytes(&expanded)) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return match rule.optional {
                    Optional::Fatal => Err(stops()),
                    Optional::Skipped | Optional::Literal => Ok(None),
                };
            }
            Err(_) => return Err(stops()),
        }
    }
    Ok(Some(expanded))
}

/// `strbuf_realpath` without `REALPATH_MANY_MISSING`: `path` resolved component by
/// component from the root or, relative, from `cwd` — empty components and `.` skipped,
/// `..` taking the last resolved component off, each link replaced by its target — with
/// only the last component allowed to be missing. `None` where git's fails.
pub(crate) fn real_path(path: &[u8], cwd: &Path) -> Option<Vec<u8>> {
    if path.is_empty() {
        return None;
    }
    let (mut resolved, mut remaining) = match path.strip_prefix(b"/") {
        Some(rest) => (b"/".to_vec(), rest.to_vec()),
        None => (cwd.as_os_str().as_bytes().to_vec(), path.to_vec()),
    };
    let mut links = 0;
    while !remaining.is_empty() {
        let start = remaining
            .iter()
            .position(|byte| *byte != b'/')
            .unwrap_or(remaining.len());
        let end = remaining[start..]
            .iter()
            .position(|byte| *byte == b'/')
            .map_or(remaining.len(), |at| start + at);
        let next = remaining[start..end].to_vec();
        remaining.drain(..end);
        match next.as_slice() {
            b"" | b"." => continue,
            b".." => {
                strip_last_component(&mut resolved);
                continue;
            }
            _ => {}
        }
        if resolved.last() != Some(&b'/') {
            resolved.push(b'/');
        }
        resolved.extend_from_slice(&next);
        match std::fs::symlink_metadata(OsStr::from_bytes(&resolved)) {
            Err(error) => {
                if error.kind() != std::io::ErrorKind::NotFound || !remaining.is_empty() {
                    return None;
                }
            }
            Ok(meta) if meta.file_type().is_symlink() => {
                if links > MAX_SYMLINKS {
                    return None;
                }
                links += 1;
                let target = std::fs::read_link(OsStr::from_bytes(&resolved)).ok()?;
                let target = target.as_os_str().as_bytes();
                let mut next_remaining = match target.strip_prefix(b"/") {
                    Some(rest) => {
                        resolved = b"/".to_vec();
                        rest.to_vec()
                    }
                    None => {
                        strip_last_component(&mut resolved);
                        target.to_vec()
                    }
                };
                if !remaining.is_empty() {
                    next_remaining.push(b'/');
                    next_remaining.extend_from_slice(&remaining);
                }
                remaining = next_remaining;
            }
            Ok(_) => {}
        }
    }
    Some(resolved)
}

/// `strip_last_component`: the last component and the separators before it, never the
/// root.
fn strip_last_component(path: &mut Vec<u8>) {
    let offset = usize::from(path.first() == Some(&b'/'));
    let mut len = path.len();
    while offset < len && path[len - 1] != b'/' {
        len -= 1;
    }
    while offset < len && path[len - 1] == b'/' {
        len -= 1;
    }
    path.truncate(len);
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    fn at(major: u32, minor: u32, patch: u32) -> GitVersion {
        version(major, minor, patch)
    }

    /// git 2.56's rule, whose every band is on.
    fn newest() -> Rule {
        Rule::of(at(2, 56, 0))
    }

    /// `is_path_owned_by_current_uid` for each effective uid, owner and `SUDO_UID`, spelled
    /// out: `SUDO_UID` is read for root alone. Caught by: gix's
    /// `is_path_owned_by_current_user`, which compares `SUDO_UID` for any effective uid —
    /// so `sudo -u bob cairn ~alice/repo` opened a repository git refuses — or root not
    /// owning what root owns, or `SUDO_UID` read before git read it.
    #[test]
    fn sudo_uid_stands_in_for_root_alone_as_git_reads_it() {
        let rule = newest();
        let sudo = |value: &str| Some(OsString::from(value));
        let cases: [(Option<u32>, u32, Option<OsString>, bool); 14] = [
            // Not root: the effective uid alone, whatever SUDO_UID says.
            (Some(1000), 1000, None, true),
            (Some(1001), 1000, None, false),
            (Some(1001), 1000, sudo("1001"), false),
            (Some(0), 1000, sudo("0"), false),
            (Some(1000), 1000, sudo("1001"), true),
            // Root: a root-owned path is owned; any other is SUDO_UID's.
            (Some(0), 0, None, true),
            (Some(0), 0, sudo("1000"), true),
            (Some(1000), 0, sudo("1000"), true),
            (Some(1000), 0, None, false),
            (Some(1000), 0, sudo("1001"), false),
            // SUDO_UID read as `strtoul` whole, cut to 32 bits; otherwise root stays root.
            (Some(1000), 0, sudo(" 1000"), true),
            (Some(1000), 0, sudo("1000x"), false),
            (Some(1000), 0, sudo("4294968296"), true),
            (Some(1000), 0, sudo("99999999999999999999999"), false),
        ];
        for (owner, euid, sudo_uid, expected) in cases {
            assert_eq!(
                owns(&rule, owner, Some(euid), sudo_uid.as_deref()),
                expected,
                "owner {owner:?}, euid {euid}, SUDO_UID {sudo_uid:?}"
            );
        }
        // A path whose owner cannot be read, and an effective uid that could not be, own
        // nothing.
        assert!(!owns(&rule, None, Some(1000), None));
        assert!(!owns(&rule, Some(1000), None, None));
        // Before git read SUDO_UID, root owns only what root owns.
        let before = Rule::of(at(2, 36, 1));
        assert!(!owns(&before, Some(1000), Some(0), sudo("1000").as_deref()));
        assert!(owns(&before, Some(0), Some(0), None));
    }

    /// This process's effective uid is read, and is the owner of a file it creates.
    #[test]
    fn the_effective_uid_is_the_owner_of_what_this_process_creates() {
        let probe = std::env::temp_dir().join(format!("cairn-euid-test-{}", std::process::id()));
        std::fs::write(&probe, b"").unwrap();
        let owner = owner_by_lstat(&probe);
        std::fs::remove_file(&probe).unwrap();
        assert!(owner.is_some());
        assert_eq!(effective_uid(), owner);
    }

    /// The effective uid is read from `/proc/self/status` and, where that cannot be read or
    /// parsed, from the owner of a file this process creates — git's `geteuid` cannot fail,
    /// so a missing `/proc` must not make every repository someone else's. Caught by: the
    /// status file as the only route on Linux.
    #[test]
    fn the_effective_uid_falls_back_to_a_created_files_owner() {
        let status = |text: &'static str| move || Some(text.to_owned());
        let created = || Some(4242);
        let nothing_created = || None;
        let unreadable = || None;
        assert_eq!(
            effective_uid_by(
                &status("Name:\tx\nUid:\t1000\t1001\t1000\t1001\n"),
                &created
            ),
            Some(1001),
            "the status file's effective uid comes first"
        );
        assert_eq!(effective_uid_by(&unreadable, &created), Some(4242));
        assert_eq!(
            effective_uid_by(&status("Name:\tx\n"), &created),
            Some(4242)
        );
        assert_eq!(
            effective_uid_by(&status("Uid:\t1000\tnot-a-number\n"), &created),
            Some(4242)
        );
        assert_eq!(effective_uid_by(&unreadable, &nothing_created), None);
    }

    /// Where the effective uid cannot be read by any route, nothing is claimed about who
    /// owns the repository: it opens where `safe.directory` names it — git opens it then
    /// whoever owns it — and is otherwise refused as [`Error::CurrentUserUnknown`], never as
    /// dubious ownership, which would say the user is not its owner. Caught by: an unknown
    /// user taken as owning nothing.
    #[test]
    fn an_unknown_user_is_never_called_someone_else() {
        let scratch = std::env::temp_dir().join(format!(
            "cairn-unknown-user-{}-{}",
            std::process::id(),
            line!()
        ));
        let _ = std::fs::remove_dir_all(&scratch);
        std::fs::create_dir_all(scratch.join(".git")).unwrap();
        let top = std::fs::canonicalize(&scratch).unwrap();
        let stop = Stop::WorkTree(top.join(".git"));
        let unknown = Identity {
            euid: None,
            owner_of: &owner_by_lstat,
        };
        let asked = Asked {
            version: at(2, 56, 0),
            executable: None,
        };
        let isolated = |name: &str| match name {
            "GIT_CONFIG_NOSYSTEM" => Some(OsString::from("1")),
            "GIT_CONFIG_GLOBAL" => Some(OsString::from("/dev/null")),
            _ => None,
        };
        match decide(&stop, &top, asked, &isolated, &unknown) {
            Err(Error::CurrentUserUnknown { path }) => assert_eq!(path, top),
            other => panic!("an unknown user was answered otherwise: {other:?}"),
        }
        // A value git stops on decides nothing either: an owner's git never reads it.
        let invalid = |name: &str| match name {
            "GIT_CONFIG_PARAMETERS" => {
                Some(OsString::from("'safe.directory=~cairn-no-such-user/x'"))
            }
            other => isolated(other),
        };
        assert!(matches!(
            decide(&stop, &top, asked, &invalid, &unknown),
            Err(Error::CurrentUserUnknown { .. })
        ));
        // Told every path is someone else's, git never asks who the user is.
        let assumed = |name: &str| match name {
            "GIT_TEST_ASSUME_DIFFERENT_OWNER" => Some(OsString::from("1")),
            other => isolated(other),
        };
        assert!(matches!(
            decide(&stop, &top, asked, &assumed, &unknown),
            Err(Error::DubiousOwnership { .. })
        ));
        let named = |name: &str| match name {
            "GIT_CONFIG_PARAMETERS" => Some(OsString::from("'safe.directory=*'")),
            other => isolated(other),
        };
        assert!(decide(&stop, &top, asked, &named, &unknown).is_ok());
        std::fs::remove_dir_all(&scratch).unwrap();
    }

    /// A `.git` file naming a directory that exists but is not a repository stops git as
    /// "not a git repository" while it reads the file, before ownership is asked about
    /// (`read_gitfile_gently`'s `is_git_directory`, then `setup.c`'s
    /// `GIT_DIR_INVALID_GITFILE`; reproduced on 2.30.9, 2.32.7, 2.38.5 and 2.56.0 under
    /// `GIT_TEST_ASSUME_DIFFERENT_OWNER=1`) — so it is [`Error::NotARepository`] here too,
    /// whoever owns what. Caught by: only an unreadable path refused, so a stranger is told
    /// of dubious ownership where git says there is no repository.
    #[test]
    fn a_gitfile_naming_no_repository_is_not_a_repository_before_ownership_is_asked() {
        let scratch = std::env::temp_dir().join(format!(
            "cairn-gitfile-no-repository-{}-{}",
            std::process::id(),
            line!()
        ));
        let _ = std::fs::remove_dir_all(&scratch);
        std::fs::create_dir_all(scratch.join("work")).unwrap();
        std::fs::create_dir_all(scratch.join("empty")).unwrap();
        let scratch = std::fs::canonicalize(&scratch).unwrap();
        let work = scratch.join("work");
        std::fs::write(
            work.join(".git"),
            format!("gitdir: {}\n", scratch.join("empty").display()),
        )
        .unwrap();
        let stranger = Identity {
            euid: owner_by_lstat(&work).map(|owner| owner.wrapping_add(1)),
            owner_of: &owner_by_lstat,
        };
        let isolated = |name: &str| match name {
            "GIT_CONFIG_NOSYSTEM" => Some(OsString::from("1")),
            "GIT_CONFIG_GLOBAL" => Some(OsString::from("/dev/null")),
            _ => None,
        };
        for found in [at(2, 30, 2), at(2, 30, 9), at(2, 56, 0)] {
            let asked = Asked {
                version: found,
                executable: None,
            };
            match decide(
                &Stop::WorkTree(work.join(".git")),
                &work,
                asked,
                &isolated,
                &stranger,
            ) {
                Err(Error::NotARepository { path }) => assert_eq!(path, work, "git {found}"),
                other => panic!("git {found}: {other:?}"),
            }
        }
        std::fs::remove_dir_all(&scratch).unwrap();
    }

    /// `:(optional)` as git 2.52 on reads it (`is_missing_file`, the same at every tag
    /// from v2.52.0 to v2.56.0): only `ENOENT` is missing; any other failure to `stat` the
    /// path — a component that is a file (`ENOTDIR`), a directory it may not search
    /// (`EACCES`) — stops git ("could not stat"; reproduced on 2.56.0). Caught by: either
    /// read as missing and skipped, or as present.
    #[test]
    fn an_optional_entry_git_cannot_stat_stops_git() {
        let scratch = std::env::temp_dir().join(format!(
            "cairn-optional-stat-{}-{}",
            std::process::id(),
            line!()
        ));
        let _ = std::fs::remove_dir_all(&scratch);
        std::fs::create_dir_all(scratch.join("locked")).unwrap();
        std::fs::write(scratch.join("afile"), b"").unwrap();
        let place = Place {
            identifies: b"/r/own",
            cwd: Path::new("/"),
            home: None,
            prefix: None,
        };
        let expanded = |found: GitVersion, path: &Path| {
            let value = format!(":(optional){}", path.display());
            expand(&Rule::of(found), value.as_bytes(), &place)
        };
        let under_a_file = scratch.join("afile/x");
        for found in [at(2, 52, 0), at(2, 53, 0), at(2, 56, 0)] {
            assert!(
                matches!(
                    expanded(found, &under_a_file),
                    Err(Error::InvalidConfig { .. })
                ),
                "git {found}"
            );
            assert_eq!(
                expanded(found, &scratch).unwrap(),
                Some(scratch.as_os_str().as_bytes().to_vec())
            );
        }
        use std::os::unix::fs::PermissionsExt as _;
        let locked = scratch.join("locked");
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
        let denied = std::fs::metadata(locked.join("x"))
            .is_err_and(|error| error.kind() == std::io::ErrorKind::PermissionDenied);
        let behind_a_lock = expanded(at(2, 56, 0), &locked.join("x"));
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
        if denied {
            assert!(
                matches!(behind_a_lock, Err(Error::InvalidConfig { .. })),
                "{behind_a_lock:?}"
            );
        } else {
            eprintln!("SKIPPED the EACCES half: this user may search a mode-000 directory");
        }
        std::fs::remove_dir_all(&scratch).unwrap();
    }

    /// Each band of the version table at its edges, maintenance releases included, spelled
    /// out against what git's source says at that tag. Caught by: any band moved by a
    /// release, or a backport forgotten.
    #[test]
    fn each_band_follows_the_version_of_git() {
        let checks = |found| Rule::of(found).checks;
        let star = |found| Rule::of(found).star;
        let sudo = |found| Rule::of(found).sudo;
        let every = |found| Rule::of(found).every_path;
        for (found, expected) in [
            (at(2, 30, 2), [false, false, false, false]),
            (at(2, 30, 3), [true, false, false, false]),
            (at(2, 30, 4), [true, true, false, false]),
            (at(2, 30, 5), [true, true, true, true]),
            (at(2, 30, 9), [true, true, true, true]),
            (at(2, 31, 1), [false, false, false, false]),
            (at(2, 31, 2), [true, false, false, false]),
            (at(2, 32, 2), [true, true, false, false]),
            (at(2, 32, 7), [true, true, true, true]),
            (at(2, 35, 1), [false, false, false, false]),
            (at(2, 35, 4), [true, true, true, true]),
            (at(2, 36, 0), [true, true, false, false]),
            (at(2, 36, 2), [true, true, true, true]),
            (at(2, 37, 0), [true, true, true, false]),
            (at(2, 37, 1), [true, true, true, true]),
            (at(2, 56, 0), [true, true, true, true]),
        ] {
            assert_eq!(
                [checks(found), star(found), sudo(found), every(found)],
                expected,
                "git {found}"
            );
        }
        for (found, prefix, command_line, includes, leading, normalised, optional) in [
            (
                at(2, 33, 8),
                false,
                false,
                true,
                false,
                false,
                Optional::Literal,
            ),
            (
                at(2, 34, 0),
                true,
                false,
                true,
                false,
                false,
                Optional::Literal,
            ),
            (
                at(2, 37, 7),
                true,
                false,
                true,
                false,
                false,
                Optional::Literal,
            ),
            (
                at(2, 38, 0),
                true,
                true,
                false,
                false,
                false,
                Optional::Literal,
            ),
            (
                at(2, 38, 5),
                true,
                true,
                false,
                false,
                false,
                Optional::Literal,
            ),
            (
                at(2, 39, 0),
                true,
                true,
                true,
                false,
                false,
                Optional::Literal,
            ),
            (
                at(2, 45, 2),
                true,
                true,
                true,
                false,
                false,
                Optional::Literal,
            ),
            (
                at(2, 45, 3),
                true,
                true,
                true,
                true,
                false,
                Optional::Literal,
            ),
            (
                at(2, 46, 0),
                true,
                true,
                true,
                true,
                false,
                Optional::Literal,
            ),
            (
                at(2, 46, 1),
                true,
                true,
                true,
                true,
                true,
                Optional::Literal,
            ),
            (
                at(2, 51, 2),
                true,
                true,
                true,
                true,
                true,
                Optional::Literal,
            ),
            (at(2, 52, 0), true, true, true, true, true, Optional::Fatal),
            (
                at(2, 53, 0),
                true,
                true,
                true,
                true,
                true,
                Optional::Skipped,
            ),
        ] {
            let rule = Rule::of(found);
            assert_eq!(
                (
                    rule.prefix,
                    rule.reading.command_line,
                    rule.reading.includes,
                    rule.leading_path,
                    rule.normalised,
                    rule.optional
                ),
                (
                    prefix,
                    command_line,
                    includes,
                    leading,
                    normalised,
                    optional
                ),
                "git {found}"
            );
        }
    }

    /// `safe.directory` as each band of git reads it, every entry against one repository
    /// whose top is `/r/own` (`/r` existing nowhere, so only the old bands match it raw,
    /// and the normalised one finds nothing to resolve). Caught by: gix's matcher (which
    /// takes `<dir>/*` to name `<dir>` itself and `/*` on any git), the empty value not
    /// resetting, `*` honoured before git had it, and an entry normalised before git
    /// normalised them.
    #[test]
    fn safe_directory_is_matched_as_each_band_of_git_matches_it() {
        let place = Place {
            identifies: b"/r/own",
            cwd: Path::new("/r/own"),
            home: Some(OsString::from("/r")),
            prefix: None,
        };
        let matches = |found: GitVersion, values: &[&str]| {
            let values: Vec<Option<Vec<u8>>> = values
                .iter()
                .map(|value| Some(value.as_bytes().to_vec()))
                .collect();
            names_the_repository(&Rule::of(found), &values, &place).unwrap()
        };
        let old = at(2, 30, 3);
        let starred = at(2, 37, 1);
        let leading = at(2, 45, 3);
        for found in [old, starred, leading] {
            assert!(matches(found, &["/r/own"]), "git {found}");
            assert!(!matches(found, &["/r/own/"]), "git {found}");
            assert!(matches(found, &["~/own"]), "git {found}");
            assert!(!matches(found, &["/r/own", ""]), "git {found}");
            assert!(matches(found, &["", "/r/own"]), "git {found}");
            assert!(!matches(found, &["own"]), "git {found}");
            assert!(!matches(found, &["."]), "git {found}");
        }
        assert!(!matches(old, &["*"]));
        assert!(matches(starred, &["*"]));
        assert!(!matches(starred, &["/r/*"]));
        assert!(matches(leading, &["/r/*"]));
        assert!(matches(leading, &["/*"]));
        assert!(
            !matches(leading, &["/r/own/*"]),
            "<dir>/* never names <dir>"
        );
        assert!(!matches(leading, &["/r/ow/*"]));
        assert!(!matches(starred, &[":(optional)/r/own"]));
    }

    /// The normalised band against real paths: links followed, a trailing slash and `.`
    /// components taken, `.` the directory git was run from, a relative entry ignored, the
    /// last component allowed to be missing and no other. Caught by: an entry compared
    /// unresolved, or resolved by `canonicalize`, which refuses a missing last component and
    /// so drops every `<dir>/*`.
    #[test]
    fn a_normalised_entry_is_resolved_as_git_resolves_it() {
        let scratch = std::env::temp_dir().join(format!(
            "cairn-safe-directory-{}-{}",
            std::process::id(),
            line!()
        ));
        let _ = std::fs::remove_dir_all(&scratch);
        std::fs::create_dir_all(scratch.join("own/sub")).unwrap();
        let scratch = std::fs::canonicalize(&scratch).unwrap();
        let own = scratch.join("own");
        std::os::unix::fs::symlink(&own, scratch.join("link")).unwrap();
        std::os::unix::fs::symlink("own", scratch.join("relative-link")).unwrap();
        let top = own.as_os_str().as_bytes();
        let place = Place {
            identifies: top,
            cwd: &own,
            home: None,
            prefix: None,
        };
        let text = |path: &Path| path.display().to_string();
        let matches = |found: GitVersion, value: &str| {
            names_the_repository(&Rule::of(found), &[Some(value.as_bytes().to_vec())], &place)
                .unwrap()
        };
        let newest = at(2, 56, 0);
        let before = at(2, 46, 0);
        for (value, normalised) in [
            (text(&scratch.join("link")), true),
            (text(&scratch.join("relative-link")), true),
            (format!("{}/", text(&own)), true),
            (text(&scratch.join("own/./sub/..")), true),
            (".".to_owned(), true),
            (format!("{}/*", text(&scratch)), true),
            (format!("{}/*", text(&scratch.join("link"))), false),
            (text(&scratch.join("missing/own")), false),
            ("own".to_owned(), false),
        ] {
            assert_eq!(matches(newest, &value), normalised, "git 2.56, {value}");
        }
        assert!(!matches(before, &text(&scratch.join("link"))));
        assert!(!matches(before, &format!("{}/", text(&own))));
        assert!(!matches(before, "."));
        assert_eq!(
            real_path(format!("{}/*", text(&own)).as_bytes(), &own),
            Some(format!("{}/*", text(&own)).into_bytes()),
            "a missing last component is kept"
        );
        assert_eq!(real_path(b"", &own), None);
        std::fs::remove_dir_all(&scratch).unwrap();
    }

    /// `git_config_pathname` per band: `%(prefix)/` from 2.34 (an absolute rest kept as
    /// it is, `system_path`'s rule), `~` and `~user` stopping git when they name nobody,
    /// `:(optional)` literal before 2.52, fatal on a missing path in 2.52 and skipped from
    /// 2.53. Caught by: an expansion git does not make, or a value git stops on accepted.
    #[test]
    fn an_entry_is_expanded_as_git_expands_it() {
        let place = Place {
            identifies: b"/r/own",
            cwd: Path::new("/"),
            home: Some(OsString::from("/home/u")),
            prefix: Some(PathBuf::from("/usr")),
        };
        let expanded = |found: GitVersion, value: &str| {
            expand(&Rule::of(found), value.as_bytes(), &place)
                .map(|path| path.map(|path| String::from_utf8_lossy(&path).into_owned()))
        };
        let newest = at(2, 56, 0);
        let some = |text: &str| Some(text.to_owned());
        assert_eq!(expanded(newest, "%(prefix)/x").unwrap(), some("/usr/x"));
        assert_eq!(expanded(newest, "%(prefix)//abs").unwrap(), some("/abs"));
        assert_eq!(
            expanded(at(2, 33, 8), "%(prefix)/x").unwrap(),
            some("%(prefix)/x")
        );
        assert_eq!(expanded(newest, "~").unwrap(), some("/home/u"));
        assert_eq!(expanded(newest, "~/x").unwrap(), some("/home/u/x"));
        assert_eq!(expanded(newest, "x~/y").unwrap(), some("x~/y"));
        assert!(matches!(
            expanded(newest, "~cairn-no-such-user/x"),
            Err(Error::InvalidConfig { .. })
        ));
        let no_home = Place {
            identifies: b"/r/own",
            cwd: Path::new("/"),
            home: None,
            prefix: None,
        };
        assert!(expand(&Rule::of(newest), b"~/x", &no_home).is_err());
        let missing = ":(optional)/cairn-no-such-path";
        assert_eq!(expanded(at(2, 51, 0), missing).unwrap(), some(missing));
        assert!(matches!(
            expanded(at(2, 52, 0), missing),
            Err(Error::InvalidConfig { .. })
        ));
        assert_eq!(expanded(at(2, 53, 0), missing).unwrap(), None);
        assert_eq!(expanded(newest, ":(optional)/").unwrap(), some("/"));
    }

    /// `GIT_TEST_ASSUME_DIFFERENT_OWNER` as `git_env_bool` reads it: unset and empty
    /// false, the words without case, an `int` with base and unit, and `None` — git stops —
    /// for anything else. Caught by: the variable read as "set means true", or a number git
    /// accepts refused.
    #[test]
    fn the_test_variable_is_a_boolean_as_git_reads_one() {
        let read = |text: &str| env_bool(Some(OsStr::new(text)));
        assert_eq!(env_bool(None), Some(false));
        for (text, expected) in [
            ("", Some(false)),
            ("1", Some(true)),
            ("0", Some(false)),
            ("TRUE", Some(true)),
            ("Yes", Some(true)),
            ("on", Some(true)),
            ("Off", Some(false)),
            ("no", Some(false)),
            (" 2", Some(true)),
            ("-1", Some(true)),
            ("0x10", Some(true)),
            ("0x0", Some(false)),
            ("010", Some(true)),
            ("1k", Some(true)),
            ("1G", Some(true)),
            ("2g", None),
            ("08", None),
            ("0x", None),
            ("maybe", None),
            ("1 ", None),
            ("2147483648", None),
            ("-2147483648", Some(true)),
            ("99999999999999999999", None),
        ] {
            assert_eq!(read(text), expected, "{text:?}");
        }
    }

    /// Which paths are asked about, for each shape git's search can stop at, recorded
    /// through the `owned` answer so no file has to change owner, and which one decides on
    /// a git that checked one path only. Caught by: the git directory a `.git` file names
    /// left unchecked (the gap a linked worktree whose `worktrees/<name>` belongs to
    /// someone else walked through), the working tree's top left unchecked, or a bare
    /// repository given a working tree to check.
    #[test]
    fn each_path_git_checks_is_asked_about_and_no_other() {
        let scratch = std::env::temp_dir().join(format!(
            "cairn-ownership-{}-{}",
            std::process::id(),
            line!()
        ));
        let _ = std::fs::remove_dir_all(&scratch);
        let top = scratch.join("tree");
        let elsewhere = scratch.join("elsewhere.git");
        std::fs::create_dir_all(&top).unwrap();
        // A git directory, as a `.git` file must name one.
        std::fs::create_dir_all(elsewhere.join("objects")).unwrap();
        std::fs::create_dir_all(elsewhere.join("refs/heads")).unwrap();
        std::fs::write(elsewhere.join("HEAD"), b"ref: refs/heads/main\n").unwrap();
        let elsewhere = std::fs::canonicalize(&elsewhere).unwrap();

        let asked = RefCell::new(Vec::new());
        let record = |refuse: Option<PathBuf>| {
            asked.borrow_mut().clear();
            let asked = &asked;
            move |path: &Path| {
                asked.borrow_mut().push(path.to_owned());
                refuse.as_deref() != Some(path)
            }
        };

        // A bare repository: the git directory, and nothing above it.
        let (owners, judged) =
            super::owners(&Stop::GitDirectory(elsewhere.clone()), &record(None)).unwrap();
        assert_eq!(*asked.borrow(), std::slice::from_ref(&elsewhere));
        assert!(owners.pass(true) && owners.pass(false));
        assert_eq!(judged, elsewhere);

        // A `.git` directory: the top and `.git`.
        let dot_git = top.join(".git");
        std::fs::create_dir(&dot_git).unwrap();
        let (owners, judged) =
            super::owners(&Stop::WorkTree(dot_git.clone()), &record(Some(top.clone()))).unwrap();
        assert_eq!(*asked.borrow(), [top.clone(), dot_git.clone()]);
        assert!(
            !owners.pass(true) && !owners.pass(false),
            "the top is someone else's"
        );
        assert_eq!(judged, dot_git);
        let (owners, _) = super::owners(
            &Stop::WorkTree(dot_git.clone()),
            &record(Some(dot_git.clone())),
        )
        .unwrap();
        assert!(!owners.pass(true));
        assert!(
            owners.pass(false),
            "a git checking one path checks only the top"
        );
        std::fs::remove_dir(&dot_git).unwrap();

        // A `.git` file: the file, the top, and the directory it names — relative, as
        // `git worktree add` writes it on git 2.48 and later with
        // `worktree.useRelativePaths`, resolved against the file's own directory.
        std::fs::write(&dot_git, b"gitdir: ../elsewhere.git\n").unwrap();
        let (owners, judged) = super::owners(
            &Stop::WorkTree(dot_git.clone()),
            &record(Some(elsewhere.clone())),
        )
        .unwrap();
        assert_eq!(
            *asked.borrow(),
            [top.clone(), dot_git.clone(), elsewhere.clone()]
        );
        assert_eq!(
            owners,
            Owners {
                gitfile: Some(true),
                work_tree: Some(true),
                git_dir: false,
            },
            "the git directory the file names is someone else's"
        );
        assert_eq!(judged, elsewhere);
        assert!(!owners.pass(true));
        assert!(owners.pass(false));

        // A `.git` file naming nothing: nothing to judge, as git stops on it.
        std::fs::write(&dot_git, b"gitdir: ../nowhere.git\n").unwrap();
        assert!(super::owners(&Stop::WorkTree(dot_git.clone()), &record(None)).is_none());

        std::fs::remove_dir_all(&scratch).unwrap();
    }

    /// Every combination of the three answers git distinguishes, spelled out rather than
    /// computed: on a git checking every path, it passes only when every path that exists
    /// is owned. Caught by: any path's answer ignored.
    #[test]
    fn every_path_must_be_owned_on_a_git_that_checks_every_path() {
        let table = [
            (None, None, true, true),
            (None, None, false, false),
            (None, Some(true), true, true),
            (None, Some(true), false, false),
            (None, Some(false), true, false),
            (Some(true), Some(true), true, true),
            (Some(true), Some(true), false, false),
            (Some(true), Some(false), true, false),
            (Some(false), Some(true), true, false),
            (Some(false), Some(false), false, false),
        ];
        for (gitfile, work_tree, git_dir, expected) in table {
            let owners = Owners {
                gitfile,
                work_tree,
                git_dir,
            };
            assert_eq!(owners.pass(true), expected, "{owners:?}");
        }
    }
}
