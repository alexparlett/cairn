#!/usr/bin/env bash
# The pre-merge gate. Exit-code safe: run this, never an ad-hoc && chain.
#   scripts/gate.sh                 full gate (the merge bar)
#   scripts/gate.sh --fast          day-loop subset (skips network-dependent checks)
#   scripts/gate.sh --step <name>   one named check (used by CI)
#
# A step left empty FAILS loudly rather than skipping: a check that silently opts
# out is not a gate. Set a variable to the literal string "skip" only for a step
# that genuinely does not apply, and record why in the comment beside it.
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
run_test_full() { run_cmd "test-full" "$TEST_FULL_CMD"; }

finish() {
  echo
  if [ "$fail" -eq 0 ]; then echo "gate: PASS"; else echo "gate: FAIL"; fi
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
else
  run_test_fast
fi

finish
