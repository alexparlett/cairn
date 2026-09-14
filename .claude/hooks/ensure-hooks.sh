#!/usr/bin/env bash
# SessionStart hook: idempotently activate the checked-in .githooks floor so the
# pre-push gate actually runs. Never clobbers an existing custom hooks setup;
# warns instead. Fails open.
set -uo pipefail

cd "${CLAUDE_PROJECT_DIR:-.}" 2>/dev/null || exit 0
git rev-parse --is-inside-work-tree >/dev/null 2>&1 || exit 0

current=$(git config --local core.hooksPath 2>/dev/null || true)
if [ -z "$current" ]; then
  git config --local core.hooksPath .githooks
elif [ "$current" != ".githooks" ]; then
  echo "note: core.hooksPath is '$current', not this repo's .githooks; the pre-push floor may not run" >&2
fi
exit 0
