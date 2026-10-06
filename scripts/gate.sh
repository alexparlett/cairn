#!/usr/bin/env bash
# The pre-merge gate. Exit-code safe: run this, never an ad-hoc && chain.
#   scripts/gate.sh                 full gate (the merge bar): every step but test-fast
#   scripts/gate.sh --fast          day-loop subset (skips network-dependent checks)
#   scripts/gate.sh --step <name>   one named check (used by CI)
#
# A step left empty FAILS loudly rather than skipping: a check that silently opts
# out is not a gate. The literal string "skip" passes a step without running it, so
# no merge-bar step may be set to it, and FAST=0 stays the default below
# (the_full_gate_is_the_default_and_no_merge_bar_step_is_skipped).
set -uo pipefail

cd "$(dirname "$0")/.." || exit 1

# ── Step commands ────────────────────────────────────────────────────────────
# Every step is workspace-wide and --all-targets, so tests and benches are held
# to the same lint floor as the binary. Tooling: rustfmt and clippy ship with
# the toolchain; `deps` needs cargo-deny (cargo install cargo-deny --locked).
FORMAT_CMD="cargo fmt --all --check"
LINT_CMD="cargo clippy --workspace --all-targets --all-features -- -D warnings"
TYPECHECK_CMD="cargo check --workspace --all-targets --all-features"
GUARDS_CMD="cargo test -p cairn-guards"                    # the invariant twins
DEPS_CMD="cargo deny check advisories bans sources licenses"
TEST_FAST_CMD="cargo test --workspace --lib --bins"        # --bins: cairn-app is a binary
TEST_FULL_CMD="cargo test --workspace --all-targets"
# --all-targets never runs doctests, and cairn-model's compile-fail pins on the
# secret type ARE doctests. Its own step, so a red test-full does not hide it.
TEST_DOC_CMD="cargo test --workspace --doc"
# cairn-git's real-git diff tests against the oldest gits Cairn supports, built from
# source (scripts/git-floor.sh). In the full sequence below, like `deps`: the first run
# fetches git's source and builds two gits (a C compiler, make and zlib's headers; the
# script names what is missing and FAILS, never skips), and later runs reuse the cached
# builds. CI runs it as its own job (ci_runs_every_merge_bar_gate_step), and
# the_local_full_gate_runs_every_step_but_the_day_loops holds the local full run to it.
GIT_FLOOR_CMD="scripts/git-floor.sh"

FAST=0
SELECTED_STEP=""
if [ "$#" -ne 0 ]; then
  case "$1" in
    --fast)
      [ "$#" -ne 1 ] && { echo "usage: scripts/gate.sh [--fast | --step <name>]" >&2; exit 2; }
      FAST=1
      ;;
    --step)
      { [ "$#" -ne 2 ] || [ -z "$2" ]; } && { echo "usage: scripts/gate.sh [--fast | --step <name>]" >&2; exit 2; }
      SELECTED_STEP="$2"
      ;;
    *)
      echo "usage: scripts/gate.sh [--fast | --step <name>]" >&2
      exit 2
      ;;
  esac
fi

fail=0
step() { echo; echo "== gate: $1"; }

# Runs one named step. An unconfigured command FAILS the gate with instructions;
# "skip" notes the deliberate omission and passes.
run_cmd() { # $1=step name, $2=command string
  step "$1"
  run_body "$1" "$2"
}

run_body() { # $1=step name, $2=command string; after the step's banner
  if [ -z "$2" ]; then
    echo "gate step '$1' is not configured. Fill its *_CMD variable in scripts/gate.sh."
    fail=1
  elif [ "$2" = "skip" ]; then
    echo "step '$1' is marked skip for this project (see scripts/gate.sh)"
  else
    echo "+ $2"
    bash -c "$2" || fail=1
  fi
}

run_format()    { run_cmd "format"    "$FORMAT_CMD"; }
run_lint()      { run_cmd "lint"      "$LINT_CMD"; }
run_typecheck() { run_cmd "typecheck" "$TYPECHECK_CMD"; }
run_guards()    { run_cmd "guards"    "$GUARDS_CMD"; }
run_deps()      { run_cmd "deps"      "$DEPS_CMD"; }
run_test_fast() { run_cmd "test-fast" "$TEST_FAST_CMD"; }
# The SSH acceptance criteria (crates/cairn-git/tests/fetch.rs) run against an
# unprivileged sshd the fixture starts; where none can be found they skip, and
# cargo hides a passing test's stderr, so the skip would read `ok`. Where the
# server is here it is REQUIRED, so a broken fixture is red; where it is not,
# the gate says so once instead of staying silent. CI installs the server and
# requires it unconditionally (.github/workflows/ci.yml). The fixture looks on
# PATH and then in the sbin directories a non-root PATH commonly lacks; the guard
# the_ssh_criteria_are_required_wherever_they_can_run pins that the two agree,
# that this runs under test-full, and that CI requires the fixture outright.
SSH_FIXTURE_NOTE=""
require_ssh_fixture_where_possible() {
  if command -v sshd >/dev/null 2>&1 || [ -x /usr/sbin/sshd ] || [ -x /usr/local/sbin/sshd ]; then
    export CAIRN_REQUIRE_SSH_FIXTURE=1
  else
    SSH_FIXTURE_NOTE="the ssh acceptance criteria in crates/cairn-git/tests/fetch.rs SKIPPED here: no sshd (install openssh-server to run them; CI requires them)"
    echo "gate: $SSH_FIXTURE_NOTE"
  fi
}

# a_read_under_the_builtin_fsmonitor_writes_only_the_daemons_own_files
# (crates/cairn-git/tests/diff/fsmonitor.rs) skips where git has no builtin
# fsmonitor daemon (Linux has one from git 2.55), and the skip would read `ok`.
# Where the git on PATH reports the daemon it is REQUIRED, so a daemon that will
# not start is red; where it does not, the gate says so once. scripts/git-floor.sh
# clears the variable for the floors' gits, which have none; the guard
# the_fsmonitor_daemon_pin_is_required_wherever_it_can_run pins all three.
FSMONITOR_NOTE=""
require_fsmonitor_daemon_where_possible() {
  if git version --build-options 2>/dev/null | grep -qx 'feature: fsmonitor--daemon'; then
    export CAIRN_REQUIRE_FSMONITOR_DAEMON=1
  else
    FSMONITOR_NOTE="the builtin-fsmonitor read test in crates/cairn-git/tests/diff/fsmonitor.rs SKIPPED here: the git on PATH has no fsmonitor--daemon"
    echo "gate: $FSMONITOR_NOTE"
  fi
}

# Two tests need a user namespace, each for something different, and skip where they
# cannot have it, which would read `ok`:
#   - the_search_crosses_a_filesystem_boundary_exactly_where_git_crosses_it
#     (crates/cairn-git/tests/diff/bare_discovery.rs) needs a second filesystem: a user
#     and mount namespace, to mount a tmpfs in. CAIRN_REQUIRE_MOUNT_NAMESPACE.
#   - the_refspec_check_sees_the_remote_of_a_repository_gix_trusts_less_than_git
#     (crates/cairn-git/tests/fetch.rs) needs a second owner: a namespace with a second
#     uid mapped (--map-auto, from /etc/subuid) whose root may give a file to it. Ubuntu
#     24.04's AppArmor (GitHub's runners) confines an unprivileged namespace in its
#     unprivileged_userns profile, which maps the uid and then refuses root the chown, so
#     the probe is that chown. CAIRN_REQUIRE_SECOND_OWNER.
# Each probe runs exactly what its test needs, so a namespace that serves one test and
# not the other requires the one and says the other skipped (user decision 2026-10-04:
# the probe tests exactly what the tests need). Where a probe succeeds its test is
# REQUIRED, so a broken namespace test is red; where it fails, the gate says so once and
# again on the PASS line. scripts/git-floor.sh leaves both variables set: a namespace owes
# nothing to git's version. The guard
# the_user_namespace_tests_are_required_wherever_they_can_run pins all of it.
MOUNTNS_NOTE=""
OWNER_NOTE=""
require_user_namespaces_where_possible() {
  if unshare --map-root-user --mount true >/dev/null 2>&1; then
    export CAIRN_REQUIRE_MOUNT_NAMESPACE=1
  else
    MOUNTNS_NOTE="the_search_crosses_a_filesystem_boundary_exactly_where_git_crosses_it (crates/cairn-git/tests/diff/bare_discovery.rs) SKIPPED here: 'unshare --map-root-user --mount' fails, so no mount namespace and no second filesystem"
    echo "gate: $MOUNTNS_NOTE"
  fi
  local probe=""
  if probe=$(mktemp -d) && : > "$probe/owned" \
    && unshare --map-root-user --map-auto chown 1:1 "$probe/owned" >/dev/null 2>&1; then
    export CAIRN_REQUIRE_SECOND_OWNER=1
  else
    OWNER_NOTE="the_refspec_check_sees_the_remote_of_a_repository_gix_trusts_less_than_git (crates/cairn-git/tests/fetch.rs) SKIPPED here: 'unshare --map-root-user --map-auto chown 1:1' fails, so no second owner (no unprivileged user namespaces, no /etc/subuid range, or a namespace whose root may not chown to the second uid, as AppArmor's unprivileged_userns profile refuses on Ubuntu 24.04)"
    echo "gate: $OWNER_NOTE"
  fi
  if [ -n "$probe" ]; then rm -rf "$probe"; fi
}

# a_reftable_repository_is_refused_at_open_and_a_files_one_opens and, for git's half,
# the_ref_storage_setting_is_read_as_git_reads_it (crates/cairn-git/tests/refs.rs) skip
# where the git on PATH cannot make a reftable repository (git before 2.45), and the skip
# would read `ok`. Where this probe makes one, with the same `git init` command the tests
# run (`git init --quiet --ref-format=reftable`), both are REQUIRED, so a refusal that
# stops working is red; where it cannot, the gate says so once and again on the PASS
# line. scripts/git-floor.sh runs no test binary that holds them. The guard
# the_reftable_refusal_is_required_wherever_it_can_run pins all of it.
REFTABLE_NOTE=""
require_reftable_where_possible() {
  local probe=""
  if probe=$(mktemp -d) && git init --quiet --ref-format=reftable "$probe/repository" >/dev/null 2>&1; then
    export CAIRN_REQUIRE_REFTABLE=1
  else
    REFTABLE_NOTE="a_reftable_repository_is_refused_at_open_and_a_files_one_opens and git's half of the_ref_storage_setting_is_read_as_git_reads_it (crates/cairn-git/tests/refs.rs) SKIPPED here: 'git init --ref-format=reftable' fails, so the git on PATH cannot make a reftable repository (git 2.45 or newer can)"
    echo "gate: $REFTABLE_NOTE"
  fi
  if [ -n "$probe" ]; then rm -rf "$probe"; fi
}

run_test_full() {
  step "test-full"
  require_ssh_fixture_where_possible
  require_fsmonitor_daemon_where_possible
  require_user_namespaces_where_possible
  require_reftable_where_possible
  run_body "test-full" "$TEST_FULL_CMD"
}
run_test_doc()  { run_cmd "test-doc"  "$TEST_DOC_CMD"; }
run_git_floor() { run_cmd "git-floor" "$GIT_FLOOR_CMD"; }

finish() {
  echo
  # A cap on coverage is restated where the verdict is read, not only where it happened.
  if [ "$fail" -eq 0 ]; then
    echo "gate: PASS${SSH_FIXTURE_NOTE:+ ($SSH_FIXTURE_NOTE)}${FSMONITOR_NOTE:+ ($FSMONITOR_NOTE)}${MOUNTNS_NOTE:+ ($MOUNTNS_NOTE)}${OWNER_NOTE:+ ($OWNER_NOTE)}${REFTABLE_NOTE:+ ($REFTABLE_NOTE)}"
  else
    echo "gate: FAIL"
  fi
  exit "$fail"
}

if [ -n "$SELECTED_STEP" ]; then
  case "$SELECTED_STEP" in
    format) run_format ;;
    lint) run_lint ;;
    typecheck) run_typecheck ;;
    guards) run_guards ;;
    deps) run_deps ;;
    test-fast) run_test_fast ;;
    test-full) run_test_full ;;
    test-doc) run_test_doc ;;
    git-floor) run_git_floor ;;
    *)
      echo "unknown gate step: $SELECTED_STEP" >&2
      exit 2
      ;;
  esac
  finish
fi

run_format
run_lint
run_typecheck
run_guards

if [ "$FAST" -eq 0 ]; then
  run_deps
  run_test_full
  run_test_doc
  run_git_floor
else
  run_test_fast
fi

finish
