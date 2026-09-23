# Hooks

Four QA layers, each doing one job at the cheapest useful boundary. Full contract:
`docs/qa-gate.md`.

| Layer | Mechanism | Cost | Blocks? |
| --- | --- | --- | --- |
| Instant debris gate | Runtime-native `qa-stop.sh` (Stop hook, every agent turn) | ms | yes |
| Hook bootstrap | `ensure-hooks.sh` (SessionStart) | ms | no |
| Session-link check | `.githooks/commit-msg`, and again over every outgoing commit in `.githooks/pre-push` | ms | yes |
| Pre-push floor | `.githooks/pre-push` | seconds | yes |
| Judgment review | `/qa` + reviewer agents | minutes | advisory |

The judgment layer is not in a hook because a hook is a shell command and cannot
reason. The debris gate is deliberately instant-only: it fires on every turn, so
anything heavier would tax every iteration.

Claude loads `.claude/settings.json`. Cairn runs Claude Code only; there is no
Codex mirror, so the Stop hook has exactly one copy and no parity obligation.

`qa-stop.sh` scans the lines the working tree adds over `HEAD` and the lines
the branch adds over where it left `main` (local or `origin/main`, whichever is
newer; if they have diverged, the local one), so debris survives a commit in its
view. A repository with neither `main` nor `origin/main` gets the uncommitted
scan only. It stays in milliseconds by
dropping, with one `grep`, every line that carries none of the rules' tokens
before `awk` sees it. `crates/cairn-guards/tests/debris_hook.rs` runs the hook
against scratch repositories in the gate; change the two together.

`qa-stop.sh`'s path-scoped rules restate the crate-layering seal from
`CLAUDE.md`. They are an approximation — the scan skips comment lines rather than
parsing Rust, so a sealed import hidden behind a trailing comment slips past. The
authority is `crates/cairn-guards/tests/invariants.rs`, which strips comments
properly and runs in the gate; the hook exists only to fail in milliseconds
instead of minutes. When you change one, change the other in the same commit.

`.githooks/commit-msg` refuses a commit message that links a Claude Code session,
because the repository is public (root `CLAUDE.md`, Conventions). `pre-push`
runs the same script over the messages of every commit it is about to send, so a
commit made with `--no-verify`, or rewritten after the fact, is still caught
before it is published. `crates/cairn-guards/tests/session_link_hook.rs` runs
the hook against scratch messages; change the two together.

## Adding a generated-file guard (pattern, for when you need it)

When the repo gains its first generated file (lockfile-adjacent manifests,
generated content tables), add a `PreToolUse` hook on `Edit|Write` that exits 2
with an explanatory message when the target path matches the generated pattern,
in the same change that introduces the file. Regenerators are unaffected because
they write via Bash.

## Trust and safety

These scripts are small, auditable, read-only apart from one idempotent
`git config --local core.hooksPath` call, and make no network calls. All fail
open: a broken guard must never wedge the edit loop. To opt out locally, remove
the hook entry from `.claude/settings.json` (debris gate) or `git config --local
--unset core.hooksPath` (pre-push floor); `git push --no-verify` bypasses the
floor in a genuine emergency.

`.claude/settings.json` also carries a `permissions.deny` block, which is NOT a
hook and is NOT opt-out: it refuses the common spellings of `gh pr merge`,
`git merge`, `git pull`, and pushes to `main`, because merging into `main` is the
user's decision and never an agent's (root `CLAUDE.md`, branch-flow bullet). It
binds Claude Code sessions only and cannot enumerate every route (`gh api …
/merge` is not closed): the rule is the authority, the deny list a backstop.
