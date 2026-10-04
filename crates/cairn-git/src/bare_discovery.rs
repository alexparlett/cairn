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
//! The search climbs no higher than the user's own git would from the same place: not into
//! a `GIT_CEILING_DIRECTORIES` ceiling, and not across a filesystem boundary unless
//! `GIT_DISCOVERY_ACROSS_FILESYSTEM` says so, both read from the launch environment
//! (`Bounds`). The search that is checked is the search that opens: [`find`] hands back
//! where it stopped, and `SharedRepository` decides ownership over exactly that
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
/// The first git whose search stops on a `.git` it cannot `stat` (but for `ENOENT` and
/// `ENOTDIR`) or that is neither a file nor a directory, rather than passing over it
/// (`read_gitfile_raw` and the switch in `setup_git_directory_gently_1`, new in v2.54.0).
const UNUSABLE_DOT_GIT_STOPS_FROM: GitVersion = version(2, 54);

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
    let bounds = Bounds::from(environment)?;
    let stop = search(start, version, &bounds)
        .map_err(|dot_git| Error::NotARepository { path: dot_git })?
        .ok_or_else(|| Error::NotARepository {
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
/// directory named `.git` is checked as itself, which git also ends up doing. It climbs no
/// higher than `bounds` let git climb: never into the longest ceiling above the starting
/// directory, and — unless `GIT_DISCOVERY_ACROSS_FILESYSTEM` says otherwise — never into
/// another filesystem. `Ok(None)` when it finds nothing, or `start` is not a directory
/// that can be read; `Err` with the `.git` the git at `version` stops on ([`dot_git`]).
fn search(start: &Path, version: GitVersion, bounds: &Bounds) -> Result<Option<Stop>, PathBuf> {
    use std::os::unix::fs::MetadataExt as _;

    let Ok(mut cursor) = std::fs::canonicalize(start) else {
        return Ok(None);
    };
    let device = |path: &Path| std::fs::metadata(path).ok().map(|meta| meta.dev());
    let Some(start_device) = device(&cursor) else {
        return Ok(None);
    };
    // Measured once, against the directory the search starts from, as git measures it.
    let ceiling = longest_ancestor_length(cursor.as_os_str().as_bytes(), &bounds.ceilings);
    loop {
        if bounds.one_filesystem && device(&cursor) != Some(start_device) {
            return Ok(None);
        }
        let dot_git = cursor.join(".git");
        if cursor.file_name() != Some(OsStr::new(".git")) {
            match self::dot_git(&dot_git, version) {
                DotGit::Repository => return Ok(Some(Stop::WorkTree(dot_git))),
                DotGit::Stops => return Err(dot_git),
                DotGit::PassedOver => {}
            }
        }
        if gix::discover::is_git(&cursor).is_ok() {
            return Ok(Some(Stop::GitDirectory(cursor)));
        }
        // git steps up to the separator before the last component and stops when that
        // separator is at or above the ceiling's end, so the ceiling itself is never
        // searched (`setup_git_directory_gently_1`'s `ceil_offset`).
        let separator = cursor
            .as_os_str()
            .as_bytes()
            .iter()
            .rposition(|byte| *byte == b'/');
        if separator.is_some_and(|at| ceiling.is_some_and(|end| at <= end)) {
            return Ok(None);
        }
        if !cursor.pop() {
            return Ok(None);
        }
    }
}

/// `GIT_CEILING_DIRECTORIES`, the variable git's search reads its ceilings from.
const CEILING_DIRECTORIES: &str = "GIT_CEILING_DIRECTORIES";
/// `GIT_DISCOVERY_ACROSS_FILESYSTEM`, the variable that lets git's search cross into
/// another filesystem.
const ACROSS_FILESYSTEM: &str = "GIT_DISCOVERY_ACROSS_FILESYSTEM";

/// How far git's search may climb, read from the launch environment as
/// `setup_git_directory_gently_1` reads it — the same at every tag from v2.30.0 to v2.56.0
/// (read at v2.30.9, v2.38.0, v2.45.0, v2.54.0 and v2.56.0). Cairn reads both for its own
/// search alone: every `git` it runs is named the repository (`--git-dir`), which no
/// ceiling or boundary stops, and neither variable is on the roster a child inherits.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Bounds {
    /// The ceilings that count, as `canonicalize_ceiling_entry` leaves them.
    ceilings: Vec<Vec<u8>>,
    /// Whether the search stops at a filesystem boundary: git's `one_filesystem`.
    one_filesystem: bool,
}

impl Bounds {
    /// Both variables as `environment` holds them. `GIT_DISCOVERY_ACROSS_FILESYSTEM` is
    /// `git_env_bool`'s: unset or false keeps the search on one filesystem, and a value it
    /// cannot read as a boolean is one git dies on before searching
    /// ([`Error::InvalidConfig`]).
    fn from(environment: &dyn Fn(&str) -> Option<OsString>) -> Result<Self, Error> {
        let across = environment(ACROSS_FILESYSTEM);
        let crosses =
            crate::ownership::env_bool(across.as_deref()).ok_or_else(|| Error::InvalidConfig {
                key: ACROSS_FILESYSTEM.to_owned(),
                value: across
                    .as_deref()
                    .map(OsStr::to_string_lossy)
                    .unwrap_or_default()
                    .into_owned(),
            })?;
        Ok(Self {
            ceilings: ceilings(environment(CEILING_DIRECTORIES).as_deref()),
            one_filesystem: !crosses,
        })
    }
}

/// `GIT_CEILING_DIRECTORIES` as git's search reads it: split at every `:`, an empty entry
/// dropped — and every entry after it kept exactly as written, unresolved — a relative
/// entry dropped, and any other resolved as `real_pathdup` resolves it (links followed,
/// `.` and `..` read, only the last component allowed to be missing), dropped where that
/// fails (`canonicalize_ceiling_entry`).
fn ceilings(value: Option<&OsStr>) -> Vec<Vec<u8>> {
    let Some(value) = value else {
        return Vec::new();
    };
    let mut empty_entry_found = false;
    let mut kept = Vec::new();
    for entry in value.as_bytes().split(|byte| *byte == b':') {
        if entry.is_empty() {
            empty_entry_found = true;
        } else if entry.first() != Some(&b'/') {
            continue;
        } else if empty_entry_found {
            kept.push(entry.to_vec());
        } else if let Some(real) = crate::ownership::real_path(entry, Path::new("/")) {
            kept.push(real);
        }
    }
    kept
}

/// `longest_ancestor_length` in git's `path.c`: the length, without a trailing `/`, of the
/// longest ceiling that is a strict ancestor of `path` — `/foo` is not one of `/foobar`, a
/// directory is not its own, and `/` has none — or `None`.
fn longest_ancestor_length(path: &[u8], ceilings: &[Vec<u8>]) -> Option<usize> {
    if path == b"/" {
        return None;
    }
    ceilings
        .iter()
        .filter_map(|ceiling| {
            let len = ceiling.len() - usize::from(ceiling.last() == Some(&b'/'));
            let ancestor =
                path.len() > len + 1 && path[..len] == ceiling[..len] && path[len] == b'/';
            ancestor.then_some(len)
        })
        .max()
}

/// What git's search makes of one directory's `.git`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DotGit {
    /// A repository: the search stops here, at a working tree.
    Repository,
    /// Nothing git can use, and the search goes on upwards.
    PassedOver,
    /// Something git stops the whole search on, as not a repository.
    Stops,
}

/// `read_gitfile_gently` as `setup_git_directory_gently_1` reads `<dir>/.git`, at
/// `version`: missing (`ENOENT`, `ENOTDIR`) it is passed over; a directory is the
/// repository when it is one and passed over when not; a regular file (links followed) is
/// read as git reads it ([`gitfile_target`]), and one that does not lead to a git
/// directory — unreadable, too large, not `gitdir: <path>`, naming nothing or naming a
/// directory that is not a repository — stops git on every version; and from 2.54 a `.git`
/// that cannot be `stat`ed otherwise, or is neither a file nor a directory, stops it too,
/// where git before passed over it. Whether a directory is a git directory is gix's
/// `is_git`, as the search's other half asks it.
fn dot_git(path: &Path, version: GitVersion) -> DotGit {
    let stops_from_2_54 = if version >= UNUSABLE_DOT_GIT_STOPS_FROM {
        DotGit::Stops
    } else {
        DotGit::PassedOver
    };
    match std::fs::metadata(path) {
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
            ) =>
        {
            DotGit::PassedOver
        }
        Err(_) => stops_from_2_54,
        Ok(meta) if meta.is_dir() => {
            if gix::discover::is_git(path).is_ok() {
                DotGit::Repository
            } else {
                DotGit::PassedOver
            }
        }
        Ok(meta) if meta.is_file() => {
            if gitfile_target(path).is_some() {
                DotGit::Repository
            } else {
                DotGit::Stops
            }
        }
        Ok(_) => stops_from_2_54,
    }
}

/// The largest `.git` file git reads: `read_gitfile_raw`'s `max_file_size`, 1 MiB at every
/// tag from v2.30.0 to v2.56.0; a larger one is "too large to be a .git file".
const MAX_GITFILE_SIZE: u64 = 1 << 20;

/// The git directory a `.git` FILE at `path` names, read as git's `read_gitfile_raw` and
/// `read_gitfile_gently` read it — the same at every tag from v2.30.0 to v2.56.0 — or
/// `None` where git stops on it: a regular file (links followed) of at most
/// [`MAX_GITFILE_SIZE`] bytes, read whole, starting `gitdir: `, with only trailing `\n` and
/// `\r` taken off (a trailing space or tab is part of the path), a path of at least one
/// byte that ends at its first NUL (git reads it as a C string), relative to the file's
/// own directory unless absolute, and naming a git directory (gix's `is_git`, as the
/// search asks of every directory). Not resolved: git's `real_path` is the caller's.
/// gix reads the same file by rules of its own — every trailing blank trimmed, at most
/// 64 KiB (gix-discover 0.55, `path::from_gitdir_file`) — so the search and the ownership
/// check ask this, never gix.
pub(crate) fn gitfile_target(path: &Path) -> Option<PathBuf> {
    use std::io::Read as _;

    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len() > MAX_GITFILE_SIZE {
        return None;
    }
    let mut contents = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(MAX_GITFILE_SIZE + 1)
        .read_to_end(&mut contents)
        .ok()?;
    if u64::try_from(contents.len()).ok()? != meta.len() {
        return None;
    }
    let mut rest = contents.strip_prefix(b"gitdir: ")?;
    while let [kept @ .., b'\n' | b'\r'] = rest {
        rest = kept;
    }
    let named = rest.split(|byte| *byte == 0).next().unwrap_or_default();
    if rest.is_empty() {
        return None;
    }
    let named = Path::new(OsStr::from_bytes(named));
    let named = if named.is_absolute() {
        named.to_owned()
    } else {
        path.parent()?.join(named)
    };
    gix::discover::is_git(&named).is_ok().then_some(named)
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

    /// No ceiling, and the search kept to one filesystem: git's search with neither
    /// variable set.
    const UNBOUNDED: Bounds = Bounds {
        ceilings: Vec::new(),
        one_filesystem: true,
    };

    fn ancestor(path: &str, ceilings: &[&str]) -> Option<usize> {
        let ceilings: Vec<Vec<u8>> = ceilings.iter().map(|c| c.as_bytes().to_vec()).collect();
        longest_ancestor_length(path.as_bytes(), &ceilings)
    }

    /// `longest_ancestor_length`, as git's `path.c` has it at every tag from v2.30.0 to
    /// v2.56.0, over the cases its own comment names. Caught by: a prefix matched without
    /// its separator (`/foo` over `/foobar`), a directory counted as its own ancestor, the
    /// trailing slash kept in the length, or the shortest ceiling winning.
    #[test]
    fn the_longest_ceiling_above_a_directory_is_measured_as_git_measures_it() {
        assert_eq!(ancestor("/a/b/c", &["/a"]), Some(2));
        assert_eq!(ancestor("/a/b/c", &["/a/"]), Some(2));
        assert_eq!(ancestor("/a/b/c", &["/a", "/a/b"]), Some(4));
        assert_eq!(ancestor("/a/b/c", &["/a/b", "/a"]), Some(4));
        assert_eq!(ancestor("/a/b/c", &["/"]), Some(0));
        assert_eq!(ancestor("/", &["/"]), None, "/ has no ancestor");
        assert_eq!(ancestor("/a/b", &["/a/b"]), None, "not its own ancestor");
        assert_eq!(
            ancestor("/a/bc", &["/a/b"]),
            None,
            "/a/b is not above /a/bc"
        );
        assert_eq!(ancestor("/a/b", &["/x", "/a/b/c"]), None);
        assert_eq!(ancestor("/a/b", &[]), None);
        assert_eq!(
            ancestor("/a//b", &["/a/"]),
            Some(2),
            "compared as bytes, as git does"
        );
    }

    fn ceilings_of(value: &str) -> Vec<String> {
        ceilings(Some(OsStr::new(value)))
            .into_iter()
            .map(|entry| String::from_utf8_lossy(&entry).into_owned())
            .collect()
    }

    /// `canonicalize_ceiling_entry`: an empty entry dropped and every entry after it kept
    /// as written; relative entries dropped; the rest resolved as `real_pathdup` resolves
    /// them, or dropped where it fails. Caught by: resolving after an empty entry, keeping a
    /// relative entry, or keeping one whose middle component is missing.
    #[test]
    fn ceiling_entries_are_read_as_git_reads_them() {
        let root = std::env::temp_dir().join(format!("cairn-ceilings-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("real")).unwrap_or_else(|e| panic!("{e}"));
        std::os::unix::fs::symlink("real", root.join("link")).unwrap_or_else(|e| panic!("{e}"));
        let root = std::fs::canonicalize(&root).unwrap_or_else(|e| panic!("{e}"));
        let r = root.display().to_string();
        assert_eq!(ceilings(None), Vec::<Vec<u8>>::new());
        assert_eq!(ceilings_of(""), Vec::<String>::new());
        assert_eq!(ceilings_of(&format!("{r}/link")), [format!("{r}/real")]);
        assert_eq!(
            ceilings_of(&format!("{r}/real/../link/")),
            [format!("{r}/real")]
        );
        assert_eq!(
            ceilings_of(&format!("{r}/missing")),
            [format!("{r}/missing")]
        );
        assert_eq!(
            ceilings_of(&format!("{r}/missing/deeper")),
            Vec::<String>::new()
        );
        assert_eq!(ceilings_of("relative:./x:../y"), Vec::<String>::new());
        assert_eq!(
            ceilings_of(&format!("{r}/link::{r}/link:relative:{r}/real/..")),
            [
                format!("{r}/real"),
                format!("{r}/link"),
                format!("{r}/real/..")
            ],
            "an empty entry leaves every entry after it as written"
        );
        assert_eq!(ceilings_of("/"), ["/"]);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// `read_gitfile_raw`'s reading, at its edges, where gix's reading differs: exactly
    /// 1 MiB is read and one byte more is not, a NUL ends the path, only `\n` and `\r` are
    /// taken off the end, a relative path is the file's directory's, and an empty one names
    /// nothing. Caught by: the limit off by one or gix's 64 KiB, a trailing blank trimmed,
    /// or a NUL kept in the path.
    #[test]
    fn a_gitfile_is_read_as_git_reads_it() {
        let root = std::env::temp_dir().join(format!("cairn-gitfile-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("refs")).unwrap_or_else(|e| panic!("{e}"));
        std::fs::create_dir_all(root.join("objects")).unwrap_or_else(|e| panic!("{e}"));
        std::fs::write(root.join("HEAD"), "ref: refs/heads/main\n")
            .unwrap_or_else(|e| panic!("{e}"));
        let gitfile = root.join("tree.git");
        let named = root.display().to_string();
        let read = |contents: &[u8]| {
            std::fs::write(&gitfile, contents).unwrap_or_else(|e| panic!("{e}"));
            gitfile_target(&gitfile)
        };
        let padded = |total: usize| {
            let mut text = format!("gitdir: {named}").into_bytes();
            text.resize(total, b'\n');
            text
        };
        assert_eq!(
            read(format!("gitdir: {named}\r\n").as_bytes()),
            Some(root.clone())
        );
        assert_eq!(read(&padded(1 << 20)), Some(root.clone()), "exactly 1 MiB");
        assert_eq!(read(&padded((1 << 20) + 1)), None, "one byte past 1 MiB");
        assert_eq!(
            read(format!("gitdir: {named}\0junk").as_bytes()),
            Some(root.clone())
        );
        assert_eq!(read(format!("gitdir: {named} ").as_bytes()), None);
        assert_eq!(read(format!("gitdir: {named}\t").as_bytes()), None);
        assert_eq!(
            read(b"gitdir: .\n"),
            Some(root.join(".")),
            "relative to the file"
        );
        assert_eq!(read(b"gitdir: \n"), None);
        assert_eq!(read(format!("gitdir:{named}").as_bytes()), None);
        assert_eq!(gitfile_target(&root), None, "a directory is no gitfile");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// `GIT_DISCOVERY_ACROSS_FILESYSTEM` as `git_env_bool` reads it: unset, empty or false
    /// keeps the search on one filesystem, true or a non-zero number lifts that, and
    /// anything else is a value git dies on. Caught by: a set-but-empty variable read as
    /// true, or a value git dies on read as either.
    #[test]
    fn crossing_filesystems_is_read_as_git_reads_a_boolean() {
        let bounds = |value: Option<&str>| {
            let value = value.map(OsString::from);
            Bounds::from(&move |name: &str| {
                (name == ACROSS_FILESYSTEM).then(|| value.clone()).flatten()
            })
        };
        let one = |value: Option<&str>| match bounds(value) {
            Ok(bounds) => bounds.one_filesystem,
            Err(error) => panic!("{value:?}: {error}"),
        };
        assert!(one(None));
        for value in ["", "0", "false", "No", "OFF"] {
            assert!(one(Some(value)), "{value:?}");
        }
        for value in ["1", "true", "YES", "on", "2", "-1", "0x10", "1k"] {
            assert!(!one(Some(value)), "{value:?}");
        }
        for value in ["sometimes", "1x", " "] {
            assert!(
                matches!(bounds(Some(value)), Err(Error::InvalidConfig { ref key, .. }) if key == ACROSS_FILESYSTEM),
                "{value:?}"
            );
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

    /// A `.git` git's search cannot use stops the search rather than being passed over, as
    /// `setup_git_directory_gently_1` stops on it: a regular file that does not lead to a
    /// git directory (here one naming an existing directory that is not a repository) on
    /// every git — "not a git repository" to 2.53, "gitfile does not point to a valid
    /// repository" from 2.54, reproduced on 2.30.9, 2.32.7, 2.38.5 and 2.56.0 — and from
    /// 2.54, whose `read_gitfile_raw` tells `ENOENT` and `ENOTDIR` from other failures, a
    /// `.git` that cannot be `stat`ed or is neither a file nor a directory (a socket here).
    /// A `.git` directory that is not a repository is passed over on every git. Each sits in
    /// a working tree inside an enclosing repository, which a search that passed over it
    /// would open. Caught by: a `.git` gix cannot follow taken as no `.git` at all.
    #[test]
    fn a_dot_git_git_stops_on_stops_the_search() {
        use std::os::unix::fs::PermissionsExt as _;
        let scratch = std::env::temp_dir().join(format!(
            "cairn-dot-git-stops-{}-{}",
            std::process::id(),
            line!()
        ));
        let _ = std::fs::remove_dir_all(&scratch);
        let outer = scratch.join("outer");
        let git_dir = outer.join(".git");
        std::fs::create_dir_all(git_dir.join("objects")).unwrap();
        std::fs::create_dir_all(git_dir.join("refs/heads")).unwrap();
        std::fs::write(git_dir.join("HEAD"), b"ref: refs/heads/main\n").unwrap();
        std::fs::create_dir_all(scratch.join("empty")).unwrap();
        let outer = std::fs::canonicalize(&outer).unwrap();
        let tree = |name: &str| {
            let path = outer.join(name);
            std::fs::create_dir_all(&path).unwrap();
            path
        };
        let named_nothing = tree("gitfile");
        std::fs::write(
            named_nothing.join(".git"),
            format!("gitdir: {}\n", scratch.join("empty").display()),
        )
        .unwrap();
        let socket = tree("socket");
        let _listener = std::os::unix::net::UnixListener::bind(socket.join(".git")).unwrap();
        let unsearchable = tree("unsearchable");
        let empty_dir = tree("dot-git-directory");
        std::fs::create_dir(empty_dir.join(".git")).unwrap();
        std::fs::set_permissions(&unsearchable, std::fs::Permissions::from_mode(0o600)).unwrap();
        let denied = std::fs::metadata(unsearchable.join(".git"))
            .is_err_and(|error| error.kind() == std::io::ErrorKind::PermissionDenied);

        let environment = |name: &str| match name {
            "GIT_CONFIG_NOSYSTEM" => Some(OsString::from("1")),
            "GIT_CONFIG_GLOBAL" => Some(OsString::from("/dev/null")),
            _ => None,
        };
        let outcome = |start: &Path, found: GitVersion| match find(start, found, &environment) {
            Ok(Stop::WorkTree(dot_git)) if dot_git == git_dir => "the enclosing repository",
            Err(Error::NotARepository { path }) if path == start.join(".git") => "stopped",
            other => panic!("git {found}, {}: {other:?}", start.display()),
        };
        for found in [at(2, 30, 9), at(2, 53, 0), at(2, 54, 0), at(2, 56, 0)] {
            let from_2_54 = found >= at(2, 54, 0);
            assert_eq!(outcome(&named_nothing, found), "stopped", "git {found}");
            assert_eq!(
                outcome(&socket, found),
                if from_2_54 {
                    "stopped"
                } else {
                    "the enclosing repository"
                },
                "git {found}"
            );
            assert_eq!(
                outcome(&empty_dir, found),
                "the enclosing repository",
                "git {found}"
            );
            if denied {
                assert_eq!(
                    outcome(&unsearchable, found),
                    if from_2_54 {
                        "stopped"
                    } else {
                        "the enclosing repository"
                    },
                    "git {found}"
                );
            }
        }
        std::fs::set_permissions(&unsearchable, std::fs::Permissions::from_mode(0o755)).unwrap();
        if !denied {
            eprintln!("SKIPPED the stat-failure half: this user may search a mode-600 directory");
        }
        std::fs::remove_dir_all(&scratch).unwrap();
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
        let git_dir = match search(root, at(2, 56, 0), &UNBOUNDED) {
            Ok(Some(Stop::WorkTree(dot_git))) => {
                gix::discover::path::from_gitdir_file(&dot_git).unwrap_or_else(|_| dot_git.clone())
            }
            _ => panic!("this checkout is not a working tree"),
        };
        assert!(matches!(
            search(&git_dir, at(2, 56, 0), &UNBOUNDED),
            Ok(Some(Stop::GitDirectory(_)))
        ));
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
