#!/usr/bin/env bash
# Stop hook: instant debris scan over ADDED lines — uncommitted ones, and those
# committed on this branch but not yet in main.
# Fires at the end of EVERY agent turn, so it must stay cheap: pure git + awk,
# no build tools, no network. The judgment layer lives in /qa, not here; a hook
# is a shell command and cannot reason. Fails open: a broken guard must never
# wedge the edit loop.
#
# Two blocks matter: FILE_PATHSPECS (what gets scanned) and the awk program
# (what counts as debris). The path-scoped rules at the bottom hold individual
# crates to the layering seal from CLAUDE.md at the cheapest possible boundary —
# they are a fast echo of the cairn-guards suite, not a replacement for it.
set -uo pipefail
# No pathname expansion: FILE_PATHSPECS is split unquoted, and `*.toml` would
# otherwise glob to the root manifests and never reach crates/*/Cargo.toml.
set -f

INPUT="$(cat 2>/dev/null || true)"
# Loop guard: if we already blocked this turn, let the stop through.
if printf '%s' "$INPUT" | grep -q '"stop_hook_active"[[:space:]]*:[[:space:]]*true'; then
  exit 0
fi

cd "${CLAUDE_PROJECT_DIR:-.}" 2>/dev/null || exit 0
git rev-parse --is-inside-work-tree >/dev/null 2>&1 || exit 0

# Rust sources and manifests. Shell and YAML are deliberately excluded: this
# file's own patterns would match themselves, and the enforcement layer has its
# own reviewer (docs/qa-gate.md) rather than a line scan.
FILE_PATHSPECS='*.rs *.toml'

# Added lines, as path:line. WORKTREE: tracked diff vs HEAD (or the empty tree
# before the first commit), plus untracked files synthesized as all-added.
# BRANCH: the working tree's diff against where this branch left main, so debris
# that was committed stays in view until it is fixed, and debris already in main
# is not re-flagged. Both are one git diff; the scan stays in milliseconds.
added_since() {
  # shellcheck disable=SC2086
  git diff "$1" --unified=0 --no-color -- $FILE_PATHSPECS 2>/dev/null | awk '
    /^\+\+\+ b\// { path = substr($0, 7) }
    /^\+/ && !/^\+\+\+/ { print path ":" substr($0, 2) }'
}
HEAD_SHA=$(git rev-parse -q --verify HEAD 2>/dev/null)
BASE=${HEAD_SHA:-$(git hash-object -t tree /dev/null)}
# shellcheck disable=SC2086
WORKTREE=$(
  {
    added_since "$BASE"
    git ls-files --others --exclude-standard -- $FILE_PATHSPECS 2>/dev/null | while IFS= read -r f; do
      [ -f "$f" ] && awk -v p="$f" '{ print p ":" $0 }' "$f"
    done
  }
)
# The newest point this branch shares with main, local or remote. If the two
# merge-bases are unrelated, the local one is kept: it may scan more, never less.
# With neither ref, FORK stays empty and only WORKTREE is scanned.
FORK=""
if [ -n "$HEAD_SHA" ]; then
  for main in refs/heads/main refs/remotes/origin/main; do
    git rev-parse -q --verify "$main" >/dev/null 2>&1 || continue
    candidate=$(git merge-base HEAD "$main" 2>/dev/null) || continue
    if [ -z "$FORK" ] || git merge-base --is-ancestor "$FORK" "$candidate" 2>/dev/null; then
      FORK=$candidate
    fi
  done
fi
BRANCH=""
[ -n "$FORK" ] && [ "$FORK" != "$HEAD_SHA" ] && BRANCH=$(added_since "$FORK")
[ -z "$WORKTREE" ] && [ -z "$BRANCH" ] && exit 0

# `strict` is 0 for lines already committed: a committed `eprintln!` or
# `#[ignore = "reason"]` is a measurement reporter someone kept, and re-flagging
# it every turn would wedge the loop. Uncommitted lines get every rule.
scan() {
  # Every rule below needs one of these tokens; grep drops the rest far faster than awk.
  printf '%s\n' "$2" |
    LC_ALL=C grep -E '<<<<<<<|>>>>>>>|!|ignore|allow|gix|freya|dioxus|cairn_git|cairn_ui|cairn_askpass|tracing|log' |
    awk -v strict="$1" '
  # Universal debris.
  /^[^:]*:<<<<<<< /               { print $0 " [merge conflict marker]" ; next }
  /^[^:]*:>>>>>>> /               { print $0 " [merge conflict marker]" ; next }
  # Rust debug debris and unfinished work left behind.
  /dbg![[:space:]]*[(\[{]/         { print $0 " [dbg! left in]" ; next }
  /todo![[:space:]]*[(\[{]/        { print $0 " [todo! left in]" ; next }
  /unimplemented![[:space:]]*[(\[{]/ { print $0 " [unimplemented! left in]" ; next }
  strict && /eprintln![[:space:]]*[(\[{]/ {
    print $0 " [eprintln! left in; delete it, or return the failure through an error type]" ; next }
  /#\[[[:space:]]*ignore[[:space:]]*\]/ { print $0 " [ignored test left in]" ; next }
  strict && /#\[[[:space:]]*ignore[[:space:]]*=/ { print $0 " [ignored test left in]" ; next }
  /#!?\[[[:space:]]*allow[[:space:]]*\([[:space:]]*(dead_code|unused)/ {
    print $0 " [blanket allow(dead_code/unused): delete the code instead]" ; next }
  # Layering seal (CLAUDE.md Invariants). The cairn-guards suite is the
  # authority (it strips comments properly); these rules just fail in
  # milliseconds instead of minutes. Prose that NAMES a sealed crate is not a
  # violation and our own docs do it constantly, so skip comment lines — the
  # cost of that approximation is a sealed import hidden behind a trailing
  # comment, which the guard suite still catches.
  {
    body = substr($0, index($0, ":") + 1)
    sub(/^[ \t]*/, "", body)
    is_comment = (body ~ /^(\/\/|\*|#)/)
  }
  is_comment { next }
  index($0, "crates/cairn-ui/") == 1 {
    if (/(^|[^A-Za-z0-9_])(gix|cairn_git)([^A-Za-z0-9_]|$)/) {
      print $0 " [cairn-ui is sealed from the git engine]" ; next }
  }
  index($0, "crates/cairn-model/") == 1 {
    if (/(^|[^A-Za-z0-9_])(gix|freya|cairn_git|cairn_ui)([^A-Za-z0-9_]|$)/) {
      print $0 " [cairn-model is plain data: no backend, no toolkit]" ; next }
  }
  index($0, "crates/cairn-git/") == 1 {
    if (/(^|[^A-Za-z0-9_])(freya|dioxus|cairn_ui)([^A-Za-z0-9_]|$)/) {
      print $0 " [cairn-git is sealed from the UI toolkit]" ; next }
  }
  # The helper runs in a process holding a plaintext secret: no engine, no
  # toolkit, and no logging framework.
  index($0, "crates/cairn-askpass/") == 1 {
    if (/(^|[^A-Za-z0-9_])(gix|freya|dioxus|cairn_git|cairn_ui|tracing|log)([^A-Za-z0-9_]|$)/) {
      print $0 " [cairn-askpass holds a plaintext secret: no engine, toolkit or logging crate]" ; next }
  }
  # Engine and askpass-channel reach only; waiting primitives are left to the guard suite.
  index($0, "crates/cairn-app/") == 1 && index($0, "crates/cairn-app/src/worker/") != 1 {
    if (/(^|[^A-Za-z0-9_])(gix|cairn_git|cairn_askpass)([^A-Za-z0-9_]|$)/) {
      print $0 " [only crates/cairn-app/src/worker may reach the git engine]" ; next }
  }
  index($0, "crates/cairn-app/src/worker/") == 1 {
    if (/(^|[^A-Za-z0-9_])(freya|dioxus)([^A-Za-z0-9_]|$)/) {
      print $0 " [the worker module runs off the UI thread: it renders nothing]" ; next }
  }'
}
HITS=$(
  {
    [ -n "$WORKTREE" ] && scan 1 "$WORKTREE"
    [ -n "$BRANCH" ] && scan 0 "$BRANCH"
  } | awk 'NF && !seen[$0]++'
)
[ -z "$HITS" ] && exit 0

# Block, listing up to 20 offending lines. Strip characters that would break
# the JSON string rather than pulling in a JSON tool.
REASON=$(
  printf '%s\n' "$HITS" | head -20 | tr -d '"\\' | tr '\t\r\n' '   ' |
    LC_ALL=C tr -d '\000-\010\013\014\016-\037'
)
printf '{"decision":"block","reason":"Debris gate (see CLAUDE.md Invariants). Fix these added lines before stopping: %s"}\n' "$REASON"
exit 0
