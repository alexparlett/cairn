#!/usr/bin/env bash
# The oldest gits Cairn supports, built from source, under cairn-git's real-git diff and
# status tests: `scripts/gate.sh --step git-floor`, part of the full gate and run by CI as
# its own job.
#
# The changes query asks the git in use (`git diff-tree`), and what that git does
# differs by version in ways the code branches on (`crates/cairn-git/src/diff/renames.rs`:
# what rename-limit check counts before 2.31, the 400 default and the 32,767 ceiling
# before 2.33); so does the status read (`git status`, which cannot read a sparse index
# before 2.32, and lazily fetches in a partial clone before 2.44:
# `crates/cairn-git/src/reads/status.rs`). The machine's own git is new, so without this
# those branches are read from git's source and never run against it. Two gits are
# built: the floor (`GitBinary::MINIMUM`, which this script checks it matches) and the
# last before 2.33, which is the only one that reaches the 2.31-2.32 branch.
#
# Each is fetched from https://github.com/git/git by its tag and checked against the
# commit pinned here, so a moved tag fails rather than building something else; built
# into a cache outside the repository (`$CAIRN_GIT_FLOOR_CACHE`, else
# `$XDG_CACHE_HOME/cairn/git-floor`, else `~/.cache/cairn/git-floor`), and reused from
# there. Building needs a C compiler, make and zlib's headers, and the first run needs the
# network; a machine without them FAILS this step, naming what is missing, rather than
# skipping it. Nothing is installed anywhere else.
#
# Seven tests skip on these gits by design, and each run prints every test it skipped, by its
# whole name, so a skip is read rather than counted as a pass:
#   - in_a_partial_clone_a_rename_search_fails_rather_than_fetching, and (in the `ops::`
#     run) a_read_in_a_partial_clone_does_not_fetch_a_missing_object, on a git older than
#     2.44, which ignores GIT_NO_LAZY_FETCH. Each fails instead if
#     CAIRN_REQUIRE_NO_LAZY_FETCH is set, so that is removed from these runs' environment,
#     and so is CAIRN_REQUIRE_FSMONITOR_DAEMON: these gits have no fsmonitor daemon.
#   - a_bare_repository_is_answered_under_safe_bare_repository_explicit and
#     a_planted_bare_repository_is_refused_at_open_and_runs_nothing (its refusal half), on
#     a git older than 2.38, which has no safe.bareRepository.
#   - a_sparse_index_is_unsupported_and_says_so, on a git older than 2.32, which cannot
#     write a sparse index (2.30.9 here).
#   - a_copy_into_an_untracked_file_is_read_from_the_untracked_commit, on a git older than
#     2.32, whose `git stash show` lists no untracked file (2.30.9 here).
#   - a_read_under_the_builtin_fsmonitor_writes_only_the_daemons_own_files, on every
#     floor: no git before 2.55 has the fsmonitor daemon on Linux.
# CAIRN_REQUIRE_MOUNT_NAMESPACE and CAIRN_REQUIRE_SECOND_OWNER, which scripts/gate.sh's
# test-full exports where its probes can make each namespace, are deliberately LEFT SET
# for these runs: the across-filesystem test in the diff_engine run needs a namespace,
# not a newer git, so where the full gate could make one the floors' gits are held to it
# too.
# Each filtered run must also list at least as many tests as its floor below, so a filter
# that stops matching — a module renamed, a test binary split — fails rather than running
# nothing and passing.
set -uo pipefail

cd "$(dirname "$0")/.." || exit 1

# version, then the commit its tag names in git.git.
VERSIONS=(
  "2.30.9 668f2d53613ac8fd373926ebe219f2c29112d93e"
  "2.32.7 b8787a98dbef7b3b1cec9818b8856de557a65256"
)

# The floor as the source states it, so raising GitBinary::MINIMUM without moving the
# first git built here fails instead of testing a floor that no longer is one.
floor_minor=$(awk '/const MINIMUM: Self = Self \{/ { inside = 1 }
  inside && /minor:/ { gsub(/[^0-9]/, ""); print; exit }' crates/cairn-git/src/process/binary.rs)
first_version=${VERSIONS[0]%% *}
if [ -z "$floor_minor" ] || [ "2.$floor_minor" != "${first_version%.*}" ]; then
  echo "git-floor: GitBinary::MINIMUM is 2.${floor_minor:-?}, but the first git built is $first_version" >&2
  exit 1
fi

# The filtered runs, as `<cargo target>|<test filters>|<floor>`. Each floor sits one under
# the run's count when it was set (141, 201, 18 and 4), so a filter that silently matches less
# fails; raise a floor as its run gains tests. `ops::` is the write verbs' argv tests, whose
# recording `git` runs the git in use (staging-and-commit phase 03), beside fetch's stubs;
# `--test diff_engine` holds the write verbs' effects too (`tests/diff/write_verbs.rs`, and
# C3 through the verbs in `tests/diff/staging.rs`), and commit and amend's against real git
# (`tests/diff/commit.rs`, phase 05). `--test status` is the working tree's status
# against its oracles (C4 of the refs-and-status packet), which owes its answer to the git
# in use as much as the diff tests do; `--test diff_engine` holds a stash's list against
# `git stash show` (C7, `tests/diff/stash.rs`), whose untracked half git 2.30.9 does not read
# and 2.32.7 does. `--test lost_commits` is Show Lost Commits against `git rev-list --reflog`
# (C20, staging-and-commit phase 10), whose fixtures write their reflogs with the git in use,
# and whose oracle that git reads.
RUNS=(
  "--lib|diff:: reads:: ops::|140"
  "--test diff_engine||200"
  "--test status||17"
  "--test lost_commits||3"
)

CACHE="${CAIRN_GIT_FLOOR_CACHE:-${XDG_CACHE_HOME:-$HOME/.cache}/cairn/git-floor}"
mkdir -p "$CACHE" || exit 1
LOGS=$(mktemp -d) || exit 1
trap 'rm -rf "$LOGS"' EXIT

# What building git needs, checked before the first fetch so a fresh machine is told
# what to install instead of failing deep inside make.
toolchain_missing() {
  local cc=${CC:-cc} missing=()
  command -v "$cc" >/dev/null 2>&1 || missing+=("a C compiler ($cc)")
  command -v make >/dev/null 2>&1 || missing+=("make")
  if command -v "$cc" >/dev/null 2>&1 &&
    ! printf '#include <zlib.h>\n' | "$cc" -E -x c - >/dev/null 2>&1; then
    missing+=("zlib's development headers")
  fi
  [ "${#missing[@]}" -eq 0 ] && return 1
  echo "git-floor: building the floor's gits needs what this machine lacks:" >&2
  printf 'git-floor:   - %s\n' "${missing[@]}" >&2
  echo "git-floor: install them, e.g. 'apt install build-essential zlib1g-dev'," >&2
  echo "git-floor: 'dnf install gcc make zlib-devel' or 'pacman -S base-devel zlib'." >&2
  echo "git-floor: this step is part of the full gate and does not skip." >&2
  return 0
}

build() { # $1=version $2=commit $3=prefix
  local version=$1 commit=$2 prefix=$3 source="$CACHE/source-$1"
  rm -rf "$source" "$prefix"
  toolchain_missing && return 1
  git init --quiet "$source" || return 1
  if ! git -C "$source" fetch --quiet --depth 1 https://github.com/git/git "refs/tags/v$version"; then
    echo "git-floor: could not fetch git v$version from github.com; the first run needs the network" >&2
    return 1
  fi
  local fetched
  fetched=$(git -C "$source" rev-parse 'FETCH_HEAD^{commit}') || return 1
  if [ "$fetched" != "$commit" ]; then
    echo "git-floor: v$version is $fetched, not the pinned $commit" >&2
    return 1
  fi
  git -C "$source" -c advice.detachedHead=false checkout --quiet FETCH_HEAD || return 1
  echo "git-floor: building git $version into $prefix"
  if ! make -C "$source" -j"$(nproc 2>/dev/null || echo 2)" prefix="$prefix" CC="${CC:-cc}" \
    NO_CURL=1 NO_OPENSSL=1 NO_GETTEXT=1 NO_TCLTK=1 NO_PERL=1 NO_PYTHON=1 NO_EXPAT=1 \
    CFLAGS="-O1 -std=gnu17 -w" install >"$CACHE/build-$version.log" 2>&1; then
    tail -n 40 "$CACHE/build-$version.log" >&2
    return 1
  fi
  rm -rf "$source"
}

fail=0
skipped_report=""
for entry in "${VERSIONS[@]}"; do
  version=${entry%% *}
  commit=${entry##* }
  prefix="$CACHE/git-$version"
  if [ ! -x "$prefix/bin/git" ] && ! build "$version" "$commit" "$prefix"; then
    echo "git-floor: could not build git $version" >&2
    fail=1
    continue
  fi
  found=$("$prefix/bin/git" --version)
  if [ "$found" != "git version $version" ]; then
    echo "git-floor: $prefix/bin/git reports '$found', not $version" >&2
    fail=1
    continue
  fi
  echo
  echo "== git-floor: cairn-git's diff tests against $found"
  for run in "${RUNS[@]}"; do
    IFS='|' read -r target filters floor <<<"$run"
    label="$target${filters:+ $filters}"
    # shellcheck disable=SC2086 # the target and the filters are words, split on purpose
    if ! listed=$(env -u CAIRN_REQUIRE_NO_LAZY_FETCH -u CAIRN_REQUIRE_FSMONITOR_DAEMON PATH="$prefix/bin:$PATH" \
      cargo test -q -p cairn-git $target -- --list $filters); then
      echo "git-floor: could not list the tests of '$label'" >&2
      fail=1
      continue
    fi
    count=$(printf '%s\n' "$listed" | grep -c ': test$')
    if [ "$count" -lt "$floor" ] || [ "$count" -eq 0 ]; then
      echo "git-floor: '$label' lists $count tests, under its floor of $floor" >&2
      fail=1
    fi
    echo "git-floor: '$label' lists $count tests (floor $floor)"
    log="$LOGS/run.log"
    # --show-output prints a passing test's stderr, which is where a skip says so.
    # shellcheck disable=SC2086
    if ! env -u CAIRN_REQUIRE_NO_LAZY_FETCH -u CAIRN_REQUIRE_FSMONITOR_DAEMON PATH="$prefix/bin:$PATH" \
      cargo test -p cairn-git $target -- --show-output $filters 2>&1 | tee "$log"; then
      fail=1
    fi
    skipped=$(grep -o 'SKIPPED [^:]*: .*' "$log" | sort -u)
    if [ -n "$skipped" ]; then
      skipped_report+="on $found, '$label' skipped:"$'\n'"$skipped"$'\n'
    else
      skipped_report+="on $found, '$label' skipped nothing"$'\n'
    fi
  done
done

# Restated where the verdict is read: a skip is coverage this step did not give.
echo
echo "== git-floor: what was skipped"
printf '%s' "$skipped_report"
exit "$fail"
