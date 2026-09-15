#!/usr/bin/env bash
# Stop hook: instant debris scan over uncommitted ADDED lines.
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

# Added lines: tracked diff vs HEAD (or the empty tree before the first commit),
# plus untracked files synthesized as all-added. Format: path:line
BASE=$(git rev-parse -q --verify HEAD 2>/dev/null || git hash-object -t tree /dev/null)
# shellcheck disable=SC2086
ADDED=$(
  {
    git diff "$BASE" --unified=0 --no-color -- $FILE_PATHSPECS 2>/dev/null | awk '
      /^\+\+\+ b\// { path = substr($0, 7) }
      /^\+/ && !/^\+\+\+/ { print path ":" substr($0, 2) }'
    git ls-files --others --exclude-standard -- $FILE_PATHSPECS 2>/dev/null | while IFS= read -r f; do
      [ -f "$f" ] && awk -v p="$f" '{ print p ":" $0 }' "$f"
    done
  }
)
[ -z "$ADDED" ] && exit 0

HITS=$(printf '%s\n' "$ADDED" | awk '
  # Universal debris.
  /^[^:]*:<<<<<<< /               { print $0 " [merge conflict marker]" ; next }
  /^[^:]*:>>>>>>> /               { print $0 " [merge conflict marker]" ; next }
  # Rust debug debris and unfinished work left behind.
  /dbg![[:space:]]*[(\[{]/         { print $0 " [dbg! left in]" ; next }
  /todo![[:space:]]*[(\[{]/        { print $0 " [todo! left in]" ; next }
  /unimplemented![[:space:]]*[(\[{]/ { print $0 " [unimplemented! left in]" ; next }
  /eprintln![[:space:]]*[(\[{]/    { print $0 " [eprintln! left in; use tracing]" ; next }
  /#\[[[:space:]]*ignore([[:space:]]*=|[[:space:]]*\])/ {
    print $0 " [ignored test left in]" ; next }
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
  # The worker partition, engine-reach half only. The waiting half needs to tell
  # handle.join() from root.join("crates"), which is the guard suite matchers job,
  # not awk with no parser.
  index($0, "crates/cairn-app/") == 1 && index($0, "crates/cairn-app/src/worker/") != 1 {
    if (/(^|[^A-Za-z0-9_])(gix|cairn_git)([^A-Za-z0-9_]|$)/) {
      print $0 " [only crates/cairn-app/src/worker may reach the git engine]" ; next }
  }
  index($0, "crates/cairn-app/src/worker/") == 1 {
    if (/(^|[^A-Za-z0-9_])(freya|dioxus)([^A-Za-z0-9_]|$)/) {
      print $0 " [the worker module runs off the UI thread: it renders nothing]" ; next }
  }'
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
