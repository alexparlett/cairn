#!/usr/bin/env bash
# The oldest gits Cairn supports, built from source, under cairn-git's real-git diff
# tests: `scripts/gate.sh --step git-floor`, which CI runs as its own job.
#
# The changes query asks the git in use (`git diff-tree`), and what that git does
# differs by version in ways the code branches on (`crates/cairn-git/src/diff/renames.rs`:
# what rename-limit check counts before 2.31, the 400 default and the 32,767 ceiling
# before 2.33). The machine's own git is new, so without this those branches are read
# from git's source and never run against it. Two gits are built: the floor
# (`GitBinary::MINIMUM`, which this script checks it matches) and the last before 2.33,
# which is the only one that reaches the 2.31-2.32 branch.
#
# Each is fetched from https://github.com/git/git by its tag and checked against the
# commit pinned here, so a moved tag fails rather than building something else; built
# into a cache outside the repository (`$CAIRN_GIT_FLOOR_CACHE`, else
# `$XDG_CACHE_HOME/cairn/git-floor`, else `~/.cache/cairn/git-floor`), and reused from
# there. Building needs a C toolchain, make and zlib's headers. Nothing is installed
# anywhere else.
#
# The partial-clone test skips on a git older than 2.44, which ignores
# GIT_NO_LAZY_FETCH, unless CAIRN_REQUIRE_NO_LAZY_FETCH is set; it is removed from the
# environment of these runs, where the skip is the right answer.
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

CACHE="${CAIRN_GIT_FLOOR_CACHE:-${XDG_CACHE_HOME:-$HOME/.cache}/cairn/git-floor}"
mkdir -p "$CACHE" || exit 1

build() { # $1=version $2=commit $3=prefix
  local version=$1 commit=$2 prefix=$3 source="$CACHE/source-$1"
  rm -rf "$source" "$prefix"
  git init --quiet "$source" || return 1
  git -C "$source" fetch --quiet --depth 1 https://github.com/git/git "refs/tags/v$version" || return 1
  local fetched
  fetched=$(git -C "$source" rev-parse 'FETCH_HEAD^{commit}') || return 1
  if [ "$fetched" != "$commit" ]; then
    echo "git-floor: v$version is $fetched, not the pinned $commit" >&2
    return 1
  fi
  git -C "$source" -c advice.detachedHead=false checkout --quiet FETCH_HEAD || return 1
  echo "git-floor: building git $version into $prefix"
  if ! make -C "$source" -j"$(nproc 2>/dev/null || echo 2)" prefix="$prefix" \
    NO_CURL=1 NO_OPENSSL=1 NO_GETTEXT=1 NO_TCLTK=1 NO_PERL=1 NO_PYTHON=1 NO_EXPAT=1 \
    CFLAGS="-O1 -std=gnu17 -w" install >"$CACHE/build-$version.log" 2>&1; then
    tail -n 40 "$CACHE/build-$version.log" >&2
    return 1
  fi
  rm -rf "$source"
}

fail=0
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
  for tests in "--lib -- diff:: reads::" "--test diff_engine"; do
    # shellcheck disable=SC2086 # each entry is the cargo arguments, split on purpose
    env -u CAIRN_REQUIRE_NO_LAZY_FETCH PATH="$prefix/bin:$PATH" \
      cargo test -p cairn-git $tests || fail=1
  done
done
exit "$fail"
