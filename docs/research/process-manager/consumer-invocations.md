# Consumers — every `git` invocation Cairn will need

Evidence record, saved in full. Commissioned 2026-10-02 by the process-manager packet planning. Read against origin/main at 2987a53 and feature/diff-engine at 44fa6a8. Historical: never retro-edited.

## What this is, and how to read it

The process manager is to be "designed and built around spawning git processes"
(`docs/research/diff-engine/rename-parity-spike.md`, "What was decided from it").
This record lists every `git` subprocess Cairn runs today or that a design
document, PRD, packet brief, phase file or open issue says it will run, so the
manager is designed against real consumers. One row per invocation, grouped by
packet in roadmap order, then the second lap from the feature inventory.

A row is only as firm as its source. Each row's last column names where it
comes from. A cell marked **(i)** is an inference from git's own documented
behaviour, not something a Cairn document says; nothing marked (i) was verified
against git's source or run for this record. "Candidate" in the invocation
column means a document implies the need but no decision picks this verb.

Source keys:

| Key | Document |
| --- | --- |
| ENG | `docs/design/engine.md` (D1) |
| CONC | `docs/design/concurrency.md` (D3) |
| CREDD | `docs/design/credentials.md` (D2) |
| CRED | `docs/systems/credentials.md` (as built) |
| OPS | `crates/cairn-git/src/ops/mod.rs` module docs (as built) |
| INV | `docs/design/feature-inventory.md` (tier noted, e.g. INV-T2) |
| RM | `docs/work/daily-loop/roadmap.md` (packet number noted) |
| DLB | `docs/work/daily-loop/brainstorm.md` (L*, O*) |
| UI | `docs/design/ui.md` and `docs/design/mockups/cairn-ui.html` |
| WT / CONF / FL | `docs/design/worktrees.md`, `conflicts.md`, `forge-links.md` |
| DPRD | `docs/prd/diff-engine.md` |
| DBS / DPROG / DSTATE | `docs/work/diff-engine/{brainstorm,progress,state}.md` on `feature/diff-engine` |
| SURV | `docs/research/diff-engine/git-process-survey.md` (feature/diff-engine) |
| SPIKE | `docs/research/diff-engine/rename-parity-spike.md` (feature/diff-engine) |
| BASE | `docs/research/diff-engine/measured-baseline.md` |
| GDA | `docs/research/diff-engine/gix-diff-api.md` |
| WCS | `docs/research/diff-engine/what-clients-show.md` |
| FORK | `docs/research/diff-engine/fork-detail-and-diff-ui.md` |
| GWP | `docs/research/backend-split/gix-write-path-coverage.md` |
| #N | GitHub issue N on `alexparlett/cairn` |

Column legend:

- **R/W, locks**: R = read, W = mutates the repository. Locks named are the
  ones git takes: `idx` = the worktree's `index.lock`; `refs` = per-ref
  `*.lock` and/or `packed-refs.lock` (in the common dir, shared by worktrees);
  `cfg` = `config.lock`; `obj` = writes objects or packs.
- **[D]**: needs `cairn_model::Confirmed` (the inventory's [D] mark, or an
  issue that says so).
- **Creds**: may ask for a credential through the askpass helper (needs an
  `AskpassToken`).
- Durations are measured numbers where a record has them (bench: rust-lang/rust
  at `c999cef531e`, Ryzen 7 9800X3D, git 2.55.0, warm), otherwise "unmeasured".

---

## 1. Built today (credential-prompts, history-graph)

| # | Invocation | R/W, locks | [D] | stdin | stdout | stderr | Duration | Cancel | Creds | Editor / hooks | Concurrency | Invalidates | Source |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| E1 | `git --version` (startup probe, `GitBinary::discover_with`) | R, none | no | none (null) | one line, captured by `run()` | error text only | `git version` floor 0.43 ms median (BASE 3.3) | not cancellable (`run()` cannot be killed) | no | none | runs per repository open on the repository thread, beside anything | nothing | SURV "binary.rs", CRED |
| E2 | `git fetch --progress --no-prune-tags --end-of-options <remote>` (`ops::fetch`) | W: `refs` (remote-tracking, `FETCH_HEAD`, `packed-refs` when `fetch.prune` prunes), `obj` (pack); may trigger auto-gc / maintenance / commit-graph write (i) | no, by policy: fetch honours `fetch.prune` for tracking refs, never prunes tags, refuses mirror or `refs/heads/*` refspecs before spawning (#17) | none (null) | discarded (`stream()` sets stdout null) | progress lines split on `\n`/`\r`, streamed to the banner one update per redraw (#25 item 1); accumulated unbounded for the error (#25 item 4); shown verbatim, never parsed | network-bound, seconds to minutes; "nothing times a fetch out" (CRED known limits) | user: `FetchCancel` → `SIGTERM`, `SIGKILL` after 2 s, stranded `*.lock` listed after the reap (#19) | yes, token per invocation | none for fetch itself; `reference-transaction` hook runs on ref updates (i) | one fetch per repository (`FetchControl` single slot; a second is silently dropped), on the operations thread, FIFO; beside any read | `refs`, `objects` declared; worker actually compares `ref_tips` before/after (two whole-ref walks, #25 item 3) | SURV, CRED, OPS, #17, #19, #25 |
| E2' | Variant proposed in #25: `git fetch --porcelain …` to read what moved from stdout instead of walking refs | as E2 | no | none | small, one machine-readable line per updated ref (i) | as E2 | as E2 | as E2 | yes | as E2 | as E2 | as E2, but exact | #25 item 3 — needs git 2.41, above the 2.30 floor (L9), "the user's call" |

History, refs and remotes reads today are gix and spawn nothing (`Repository::remotes`,
`ref_tips`, `HistorySession`). #4 (retained history rows) is not a process
consumer: its backward-paging gap is a gix walk question.

## 2. diff-engine (packet 3, paused for this packet)

| # | Invocation | R/W, locks | [D] | stdin | stdout | stderr | Duration | Cancel | Creds | Editor / hooks | Concurrency | Invalidates | Source |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| D1 | **Changes query, option E**: `git diff-tree -r -M -z --raw --no-abbrev --no-ext-diff [-C] [-l<n>] <old> <new>` — a commit against its first parent (L5: a merge diffs against `^1`), a root commit against the empty tree (`--root` or the empty tree id (i)), or two commits tip against tip (L7 compare) | R, no locks (diff-tree never reads the index (i)); spike ran with `GIT_OPTIONAL_LOCKS=0 LC_ALL=C` | no | none | large `-z` records: S1 27,592 records / 5.5 MB; parse 0.1–0.6 ms, 4.6 ms on S1 | rename-limit warning possible ("exhaustive rename detection was skipped due to too many files", BASE); never fired at defaults on the bench; R2.2 wants a cut-short limit *reported* but the output policy says stderr is never parsed | 2.4 ms process floor; git spawn→output 7.4–37.5 ms typical, 82.7 ms worst (M1); fallback-sample p50 9.3 ms, p90 19 ms (SPIKE §1–2); name-status S1 28.6 ms, M1 78.5 ms (BASE 3.1) | epoch, changes lane (a changes query also supersedes the file-diff lane, L8); needs a bridge from pull-based `Cancel` to push-based `ProcessKill` (SURV gap 6) | no — token-less environment, askpass fails closed | none; `--no-ext-diff` keeps a configured external diff out | diff thread (L8), which holds no `GitBinary` today (SURV "How a read-path spawn would have to be wired"); runs beside writes and reads | nothing; answer may be cached by tree ids (R4.5) | SPIKE "What was decided" (option E), DPROG 2026-09-30, DSTATE, BASE, SURV |
| D2 | Candidate: working-tree change lists that phase 03 "builds on" the manager for — `git diff-index --cached -M -z --raw HEAD` (staged, with renames), `git diff-files -z --raw` (unstaged), or both folded into status (P4) | R; `GIT_OPTIONAL_LOCKS=0` needed or a refresh may write `idx` (BASE method) | no | none | `-z` records, one per changed path | as D1 | unmeasured; W1 status (29.5 ms clean) is the nearest number | epoch | no | `core.fsmonitor` may run (i) | diff thread | nothing; never cached (L8: working-tree answers are never cached) | DPROG 2026-09-30 ("phase 03's working-tree reads and packet 5's staging build on it too"); R3 itself is gix (DPRD R3, DBS L6) — which verb, if any, is undecided |
| D3 | Not a manager consumer: the user's **clean filter driver** (and gix's long-running `process` filter clients held in `gix_filter::Pipeline`), started by gix during a working-tree read | R | no | file content | converted content | inherited from Cairn | per file; LFS/git-crypt drivers can be slow (unmeasured) | gix's, not Cairn's | n/a | n/a | wherever the diff thread runs | n/a | ENG "Reads see git's form" (stated residual: inherited environment and stderr), DBS L6, GDA §5–6, §8 |
| D4 | Not a manager consumer: test fixtures — `git apply --cached`, `git write-tree`, `git update-index` (C1–C3, into a scratch `GIT_INDEX_FILE`), `git diff -U3`, `git diff --cached`, `git diff`, `git diff --no-index` (C6, C7), `git diff-tree -r --raw --no-abbrev` (C5) | test-only | — | patches | parsed by tests | — | — | — | — | — | — | — | DPROG phase 02, DPRD C1–C7; the terminal-prompt guard blanks test modules, "a test fixture may spawn what it likes" (root CLAUDE.md) |

Phase 02's gix changes query (`crates/cairn-git/src/diff/changes.rs`, including
`repair_copies` and `RenameDetection`) is superseded by D1; the content query
stays gix (DPROG 2026-09-30).

## 3. refs-and-status (packet 4)

| # | Invocation | R/W, locks | [D] | stdin | stdout | stderr | Duration | Cancel | Creds | Editor / hooks | Concurrency | Invalidates | Source |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| S1 | Candidate, if O2 picks git: `git status --porcelain=v2 -z [--untracked-files=all] [--branch] [--show-stash]` (flags (i)) | R only with `GIT_OPTIONAL_LOCKS=0`; without it status may refresh and rewrite `.git/index` (BASE method) | no | none | `-z` records, one per changed, untracked or conflicted path — unbounded on a dirty tree (a build directory of untracked files) | warnings | W1 clean, 62,892 entries: 29.5 ms median, multi-threaded (18.1 ms user / 45.7 ms sys) (BASE 3.3); dirty unmeasured — packet 4's L6 bar is "status on a large, dirty working tree" (RM 4) | epoch (a newer status supersedes) | no | runs `core.fsmonitor` (i) and clean filters for racily-clean entries (RM 4 note applies to gix too) | "the query a client runs most often" (RM 4); beside reads; a run concurrent with an index writer sees a transient state | nothing; invalidated by `index`, `working_tree` | RM 4, DLB O2, BASE, OPS output policy (`--porcelain=v2` named for status) |
| S2 | Not a consumer: refs enumeration (branches, remotes, tags), stashes, history from every ref (`HistoryRequest::from_commits`) — gix | R | — | — | — | — | — | — | — | — | — | — | RM 4, INV-T0 |

## 4. staging-and-commit (packet 5)

| # | Invocation | R/W, locks | [D] | stdin | stdout | stderr | Duration | Cancel | Creds | Editor / hooks | Concurrency | Invalidates | Source |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| P1 | Stage file(s): `git add -- <paths>`, or `git add --pathspec-from-file=- --pathspec-file-nul` for a long selection (i; git 2.26+, inside the 2.30 floor) | W: `idx`, `obj` (blobs) | no | optional NUL-separated pathspec list (thousands of paths for "stage all") | none | CRLF/filter warnings | unmeasured; index read+rewrite (7.6 MB index on the bench) plus hashing; clean filters (LFS) can take seconds per large file (i) | kill mid-write strands `index.lock` (i) | no (i) | clean filters run (the reason writes go through git, ENG) | exclusive on the worktree's index | `index`, `objects` | INV-T2, RM 5 |
| P2 | Unstage file(s): `git restore --staged -- <paths>` or `git reset -q -- <paths>` (i) | W: `idx` | no | optional pathspec list | none | errors | unmeasured | as P1 | no | none | exclusive on index | `index` | INV-T2, RM 5 |
| P3 | Stage hunk / lines: `git apply --cached --check -` then `git apply --cached -` (what `git add -p` runs; no `--recount`, `--unidiff-zero` or `-p0`). GitHub Desktop adds `--unidiff-zero --whitespace=nowarn` | W: `idx` (the `--check` pass writes nothing (i)) | no | **the patch** from `emit_patch`, always three lines of context; KB typically, bounded by `DiffLimits` (64 MiB per version with Load Diff) | none | "patch does not apply" and friends — must reach the user | unmeasured | short; killing risks `index.lock` | no | none | exclusive on index; a write then read in one request must reopen the handle (same-timestamp-tick residual, OPS `index`) | `index` | RM 5 ("`git apply --cached` needs a runner change, which puts `destructive-ops-reviewer` on that phase"), DLB L2, DLB L3, `docs/design/diff.md`, GDA §10, WCS Finding 16 |
| P4 | Unstage hunk / lines: `git apply --cached -R -` (`add -p`'s reset mode is `{"-R","--cached"}`) | W: `idx` | no | patch | none | as P3 | unmeasured | as P3 | no | none | as P3 | `index` | GDA §10, diff.md ("the same patch in reverse removes exactly the selection") |
| P5 | Discard hunk / lines: `git apply -R -` against the working tree (`add -p`'s discard modes), or Desktop's inverted patch through `git apply --unidiff-zero --whitespace=nowarn -` | W: working tree; may read and refresh index (i) | **yes** — uncommitted work, no recovery at all | patch | none | apply errors | unmeasured | as P3 | no | smudge-side conversion on write (i) | exclusive with every index/worktree writer | `working_tree` | INV-T2 [D], INV "Recovery", RM 5, GDA §10, WCS Finding 16 |
| P6 | Discard file: `git restore --worktree -- <paths>` / `git checkout -- <paths>` (i) | W: working tree (and `idx` refresh (i)) | **yes** | optional pathspec list | none | errors | unmeasured | as P1 | an LFS smudge may fetch objects over the network and ask for a credential (i) | smudge filters, LFS | exclusive | `working_tree` | INV-T2 [D] |
| P7 | Clean untracked: `git clean -f [-d] -- <paths>` (i); candidate read `git clean -n` to list what the prompt will name (i) | W: working tree | **yes** | none | `-n`: one line per path (i) | errors | unmeasured | as P1 | no | none | exclusive with worktree writers | `working_tree` | INV-T2 [D]; the "say what will be lost, how much" rule (cairn.md, UI) |
| P8 | Commit: `git commit -F - [--signoff] [--cleanup=…]` (flags (i); Sign-off is in the mockup) | W: `idx` (git holds it across the pre-commit hook and points the hook at the locked index (i, unverified)), `refs` (HEAD's branch + reflog), `obj` | no | the message (KB) — `-F -` keeps an editor from opening (i) | summary line, not parsed | **hook output** — linters can print MBs; must be shown, and streaming it is what tells a user the hook is still running | ms without hooks; **unbounded with hooks** (pre-commit frameworks, seconds to minutes) — unmeasured | user cancel of a hung hook is the useful one; the hook is git's child, so a `SIGTERM` to git alone may leave it running (i) → process groups (SURV gap 9) | GPG when `commit.gpgSign` is set → `gpg-agent`/pinentry, which needs `GNUPGHOME`/`DISPLAY` that #18 leaves undecided | **must never open an editor**: pin `GIT_EDITOR` to fail closed "the moment a verb that can open an editor lands, in that verb's change" (#18); hooks `pre-commit`, `prepare-commit-msg`, `commit-msg`, `post-commit`, `reference-transaction`; hooks may re-stage files (Fork's partial-stage reports, FORK) or run git themselves (i) | exclusive on index and HEAD; reads during hooks see churn | `refs`, `index`, `objects` (+ `working_tree` if a hook rewrote files) | INV-T2 ("Must run hooks"), ENG, RM 5, UI mockup note 4 ("pre-commit hook will run"), #18, FORK |
| P9 | Amend: P8 with `--amend` | as P8 | **yes** (old commit survives only in the reflog); the reflog view ships with it (DLB L4) | as P8 | as P8 | as P8 | as P8 | as P8 | as P8 | as P8 | as P8 | as P8 | INV-T2 [D], RM 5, DLB L4 |
| P10 | Stash create: `git stash push [-m <msg>] [--include-untracked] [--keep-index] [-- <paths>]` (flags (i)); also the mechanism if O3 decides to auto-stash before a discard | W: `idx`, `refs` (`refs/stash` + reflog), `obj`, working tree | no | none | small | messages | unmeasured; proportional to the dirty tree | as P1 | no | smudge on the reset half (i) | exclusive | `refs`, `index`, `working_tree`, `objects` | INV-T2, RM 5, DLB L5/O3, UI ("stash first" in the discard dialog) |
| P11 | Stash apply / pop: `git stash apply [--index] stash@{n}`, `git stash pop …` (i) | W: all of the above; can stop with conflicts (exit 1 with the stash kept, for pop) (i) | no | none | small | conflict messages | unmeasured | not safely mid-way | no | none | exclusive | all four | INV-T2, RM 5 |
| P12 | Stash drop: `git stash drop stash@{n}` | W: `refs` (`refs/stash` reflog) | **yes** | none | one line | errors | ms (i) | n/a | no | none | exclusive on refs | `refs` | INV-T2 [D], RM 5 |
| P13 | Not a git consumer: `.gitignore` editing (a file write — whether it counts as an `ops` mutation is unstated); reflog view and stash list (gix reads); operation log (fed by `Performed`) | — | — | — | — | — | — | — | — | — | — | — | INV-T2, INV-T7, DLB L4 |

## 5. remote-sync (packet 6)

| # | Invocation | R/W, locks | [D] | stdin | stdout | stderr | Duration | Cancel | Creds | Editor / hooks | Concurrency | Invalidates | Source |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| N1 | Push: `git push --progress [--porcelain] [--set-upstream] [--force-with-lease=<ref>:<oid> \| --force] --end-of-options <remote> <refspec>` ("a `GitCommand` with `--progress --end-of-options`, streamed stderr, a `Send` canceller, a `Performed`", #16; `--porcelain` (i)) | W: the remote; local `refs` (tracking refs after success), `cfg` with `-u` (i) | **yes** for force, force-with-lease, delete, non-fast-forward; nothing for a plain fast-forward of the current branch (#16) | none | `--porcelain`: one status line per ref (i) | progress, plus `remote:` messages (a forge's "create a pull request" line) to show | network, seconds to minutes, **plus `pre-push`**, which can run a test suite | user, as E2 | **yes**, "the token whatever the configuration" (#16 quoting `fetch.rs`) | `pre-push` (git feeds it the refs on its stdin (i)), `reference-transaction` | as E2's slot today; whether a push may run beside a fetch is unstated | `refs` | #16, RM 6, INV-T5, CRED L8, FL (push-and-create-PR) |
| N2 | Pull: O4 merge or rebase — `git pull --no-rebase\|--rebase [--ff-only] <remote> <branch>`, or Cairn composing E2 + M1/M3 (i) | W: everything; may stop on conflicts | inventory does not mark it; a rebase pull rewrites local commits | none | small | progress, then merge/rebase messages | network + merge; unmeasured | safe in the fetch half, not in the merge half (i) | yes | must not open an editor for a merge commit (`--no-edit` (i)); `post-merge`, `post-rewrite`, `post-checkout` | as N1 | all four | RM 6 (O4), DLB O4, INV-T5 |
| N3 | Upstream: `git branch --set-upstream-to=<remote>/<b> [<b>]`, `--unset-upstream` (i) | W: `cfg` | no | none | none | errors | ms (i) | n/a | no | none | config writer | config (not an `Invalidated` flag today) | RM 6, INV-T3 ("Set upstream") |
| N4 | Remote add / edit / remove: `git remote add <n> <url>`, `git remote set-url …`, `git remote rename …`, `git remote remove <n>` (i) | W: `cfg`; remove and rename also delete or rewrite tracking `refs` (i) | no (inventory) | none | none | errors | ms; a rename over many refs longer (i) | n/a | no | none | config + refs writer | `refs`, config | RM 6, INV-T5 |
| N5 | Prune: `git remote prune <remote>` or `git fetch --prune`; candidate read `git remote prune --dry-run <remote>` to list what the prompt names (i) | W: `refs` (dry-run: R, but contacts the remote) | **yes** | none | one line per pruned ref (i) | errors | network | user | yes | none | as E2 | `refs` | INV-T5 [D], RM 6, #17 ("prune as a confirmed operation listing what it would delete") |
| N6 | Push tags / delete remote tag: `git push <remote> <tag>`, `git push <remote> --delete refs/tags/<t>` (i) | as N1 | **yes** for delete (INV-T5, RM 6) | none | as N1 | as N1 | as N1 | as N1 | yes | `pre-push` | as N1 | `refs` | INV-T5 [D], RM 6 |
| N7 | Clone: `git clone --progress -- <url> <dir>` (i) | W: creates a repository — **no repository exists to key the invocation by**; cwd is a parent directory | no | none | none | progress | network, minutes on a large repository; then a full checkout (smudge/LFS) | user; a cancelled clone's partial directory (i) | yes | `post-checkout` | no repository to serialize against | — | INV-T5 (not in RM 6's brief) |
| N8 | Not git, but a spawn: forge links "hand it to the system opener" (`xdg-open`, macOS `open` (i)) | — | no | — | — | — | fire and forget | — | no | — | — | — | FL, UI, RM 6 — the terminal-prompt guard forbids naming `Command` outside `ops/environment.rs`, so where this spawn lives is a question |

## 6. branch-ops (packet 7)

| # | Invocation | R/W, locks | [D] | stdin | stdout | stderr | Duration | Cancel | Creds | Editor / hooks | Concurrency | Invalidates | Source |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| B1 | Create branch: `git branch [--track\|--no-track] <name> <start>` or `git switch -c …` (i) | W: `refs`, `cfg` with tracking | no | none | none | errors | ms (i) | n/a | no | `reference-transaction` (i) | refs writer | `refs` | INV-T3, RM 7 |
| B2 | Rename branch: `git branch -m <old> <new>` (i) | W: `refs` + reflog + `cfg` | no | none | none | errors | ms (i) | n/a | no | as B1 | refs + config writer | `refs` | INV-T3, RM 7 |
| B3 | Delete branch: `git branch -d\|-D <name>` (i) | W: `refs` | **yes** (unmerged commits become unreachable); the count for the prompt is a gix read | none | one line | errors | ms (i) | n/a | no | as B1 | refs writer | `refs` | INV-T3 [D], RM 7 |
| B4 | Checkout / switch: `git switch <branch>` / `git checkout <branch>` (i) | W: `idx`, HEAD, working tree | **yes when the working tree is dirty** — "exactly when a user does not expect it" | none | none | messages | proportional to the tree delta; a far switch on a 62,880-file tree is unmeasured | **not safe mid-way** — a half-updated working tree (i) | an LFS smudge may fetch and ask (i) | `post-checkout`; smudge filters; sparse checkout and submodule recursion honoured (ENG's reasons for D1) | exclusive on index and worktree; refused for a branch checked out in another worktree (WT; git refuses too (i)) | `refs`, `index`, `working_tree` | INV-T3 [D], RM 7, WT, ENG |
| B5 | Create tag: `git tag <name> <commit>`; annotated `git tag -a -F - <name> <commit>` (i) | W: `refs`, `obj` (annotated) | no | annotated: the message | none | errors | ms (i) | n/a | GPG under `tag.gpgSign` (i) — #18's `GNUPGHOME` | none | refs writer | `refs`, `objects` | INV-T3, RM 7 |
| B6 | Delete tag: `git tag -d <name>` | W: `refs` (git never reflogs a tag, #17) | **yes** | none | one line | errors | ms (i) | n/a | no | none | refs writer | `refs` | INV-T3 [D], RM 7 |

## 7. worktrees (packet 8)

| # | Invocation | R/W, locks | [D] | stdin | stdout | stderr | Duration | Cancel | Creds | Editor / hooks | Concurrency | Invalidates | Source |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| K1 | List: gix ("cheap on the read side"); fallback `git worktree list --porcelain` (i; its `-z` needs git 2.36, above the floor (i)) | R | no | none | small records | — | ms | epoch | no | none | beside anything | — | WT, RM 8 |
| K2 | Create (O5 open): `git worktree add [-b <new>] <path> <commit-ish>` (i) | W: admin dir under the common dir, `refs` with `-b`, a full checkout into a new directory | no | none | none | progress-like messages | a full checkout (seconds on a large repository, unmeasured) | not safe mid-way (partial worktree) (i) | LFS smudge (i) | `post-checkout`, smudge filters | shares refs/objects with every worktree; its own new index | worktree list, `refs` | RM 8 (O5), DLB O5, WT |
| K3 | Remove: `git worktree remove [--force] <path>` (i) | W: deletes the directory and its admin dir | **yes when `--force`** removes uncommitted or untracked work (i; not marked in INV) | none | none | errors | proportional to the tree | n/a | no | none | conflicts with any process in that worktree | worktree list | WT, RM 8 |
| K4 | `git worktree prune`, `lock [--reason]`, `unlock`, `move` (i) | W: admin dirs | no | none | none | errors | ms | n/a | no | none | — | worktree list | WT ("list, create, switch and remove"; prune/lock/move are (i)) |

## 8. Tier 2+ second lap (feature inventory, roadmap "After D7")

| # | Invocation | R/W, locks | [D] | stdin | stdout | stderr | Duration | Cancel | Creds | Editor / hooks | Concurrency | Invalidates | Source |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| M1 | Merge: `git merge [--no-ff\|--ff-only] --no-edit <commit>` (or `-F -`), then `git merge --continue` / `--abort` (i) | W: `idx`, `refs`, working tree, `obj`; **can stop with conflicts (exit 1, `MERGE_HEAD` written) — a stop, not a failure** (i) | Tier 4: "Every one of these needs `Confirmed`"; `--abort` discards resolution work (i) | message if `-F -` | small | conflict list in prose (never parsed, OPS) | proportional to the change; unmeasured | not safe mid-way; abort is the cancel | no | must not open an editor; `pre-merge-commit`, `commit-msg`, `post-merge` | exclusive; while MERGING only continue/abort/resolve are valid | all four | INV-T4, CONF, RM "After D7" |
| M2 | Conflict resolution writes: write the chosen regions to the file, then `git add -- <path>`; `git checkout --ours\|--theirs -- <path>`; `git rm -- <path>` for delete/modify (i) | W: working tree, `idx` | per region, no (i) | optional pathspec list | none | errors | ms (i) | n/a | no | "edit in your editor" — an external editor process, minutes | exclusive on index | `index`, `working_tree` | CONF (D6), UI |
| M3 | Rebase: `git rebase [--onto <new>] <upstream> [<branch>]`, `--continue`, `--skip`, `--abort` | W: everything; per-commit replay; stops on conflict | **yes** (Tier 4) | none | small | messages | minutes on a long branch (i); unmeasured | abort is the cancel; a kill leaves rebase state (i) | no | `--continue` may open an editor for a message (i) → `GIT_EDITOR` policy (#18); `pre-rebase`, `post-rewrite`, `post-checkout` | exclusive; the repository is mid-operation until done | all four | INV-T4 ("Abort / continue / skip state machine") |
| M4 | Interactive rebase (its own program): `git rebase -i <base>` with `GIT_SEQUENCE_EDITOR` naming a Cairn-provided program that writes the todo the UI built, and `GIT_EDITOR` for reword/squash messages (i); `exec` lines run arbitrary commands | as M3 | **yes** | the todo, through the editor program rather than stdin (i) | small | messages | **blocks on the UI** while a todo or message is edited — interactive, minutes | as M3 | no | the sequence editor and editor ARE the protocol | as M3 | all four | INV-T4 ("a subsystem"), cairn.md "Still open", RM "After D7" |
| M5 | Cherry-pick: `git cherry-pick [-x] [-m 1] <commits>` + `--continue/--skip/--abort` (i) | as M3 | Tier 4 | none | small | messages | per commit | as M3 | no | as M3 | as M3 | all four | INV-T4 |
| M6 | Revert: `git revert --no-edit <commit>` + continue/abort (i) | as M5 | Tier 4, "the one non-destructive member" | none | small | messages | per commit | as M3 | no | `--no-edit` (i) | as M3 | all four | INV-T4 |
| M7 | Reset: `git reset --soft\|--mixed <c>` (recoverable via reflog); `git reset --hard <c>` | W: `refs`, `idx`; `--hard` also working tree | **`--hard` yes** ("Loses uncommitted work with no recovery at all"); soft/mixed in Tier 4 | none | small | messages | proportional to the tree (hard) | not safe mid-way (hard) (i) | no | none (i) | exclusive | `refs`, `index` (+ `working_tree`) | INV-T4 [D] |
| M8 | Squash / fixup: through M4 with `--autosquash`, or `git commit --fixup <c>` then M4 (i) | as M4 | **yes** | as M4 | as M4 | as M4 | as M4 | as M4 | as M4 | as M4 | as M4 | all four | INV-T4 [D] |
| M9 | Candidate: blame — the inventory says "`gix-blame` is available"; the user's git-parity rule and option E's precedent could put it on `git blame --incremental\|--porcelain -M -C <rev> -- <path>` (i) | R | no | none | **streaming** records, one per line group | — | seconds on a deep file (i); unmeasured | epoch | no | none | beside anything | — | INV-T1; SPIKE (the precedent) |
| M10 | Candidate: file / path history with rename following — `git log --follow -z --format=… -- <path>` (i) if gix's walk cannot match `--follow` | R | no | none | **streaming**, unbounded on a long history | — | proportional to history depth; unmeasured | epoch | no | none | beside anything | — | INV-T1 |
| M11 | Candidate: commit search pickaxe — `git log -S<s>\|-G<re> --format=%H -z` (i); `--grep`, author and date can stay gix | R | no | none | **streaming** hits | — | a blob diff per commit: minutes on rust-lang/rust (i); unmeasured | epoch — must stop the walk, not finish it | no | none | beside anything | — | INV-T1 ("A differentiator") |
| M12 | Submodules: status (gix or `git submodule status`), `git submodule update --init [--recursive]`, `sync` (i) | W (update) | no | none | small | progress | network, per submodule | user | yes — "submodule credentials" are outside credential-prompts' scope (CRED) | `post-checkout` | as a fetch per submodule | `working_tree`, submodule repositories | INV-T1, INV-T6 |
| M13 | LFS: "Needs verifying, not building" — the smudge/clean filters run inside other verbs; explicit `git lfs pull/fetch` is not designed (i) | W | no | — | — | progress | network | user | yes | — | — | `working_tree` | INV-T6, ENG |
| M14 | Init: `git init [<dir>]` (i) | W: creates a repository — no key | no | none | one line | errors | ms | n/a | no | none | — | — | INV-T6 |
| M15 | Config editing: `git config [--local] <key> <value>`, `--unset` (i) | W: `cfg` | no | none | none | errors | ms | n/a | no | none | config writer | config | INV-T6 |
| M16 | Candidate read closing a stated residual: asking git for its configuration origins, e.g. `git config --list --show-origin --show-scope -z` (i) | R | no | none | small `-z` records | — | ms | epoch | no | none | beside anything | — | CRED / #17 ("asking git for its system config path would spawn a process outside GitEnvironment") |
| M17 | Not git, but spawns: open in terminal / editor, external diff / merge tool (Tier 7) | — | — | — | — | — | long-lived GUI processes | — | — | — | — | — | INV-T7, CONF |
| M18 | Out of scope: `git bisect`, `filter-branch` / `filter-repo` | — | — | — | — | — | — | — | — | — | — | — | INV "Explicitly out of scope" |

## 9. Processes that start without Cairn building them

Each of these is a process alive in Cairn's process tree, or started because
Cairn ran git, that no registry would see today.

| Process | Started by | Why it matters to the manager | Source |
| --- | --- | --- | --- |
| `ssh`, `git-remote-https`, `cairn-askpass` | git, during E2/N1 | not in a process group; survive a kill of git and hold the stderr pipe, parking the reader thread (#25 item 4) | SURV, CRED known limits, #25 |
| Hooks (`pre-commit`, `pre-push`, …) and anything they run | git, during P8/N1/M1–M5/B4 | unbounded duration; may run git themselves; a cancel must reach them | ENG, INV-T2, FORK |
| Clean filter drivers, long-running `process` filters | gix, on a read | inherited environment and stderr (stated residual) | ENG, GDA |
| `git gc --auto` / `git maintenance run --auto`, detached by default | git, after fetch/commit/merge (i) | outlives the invocation that triggered it; holds locks (`gc.pid`, `packed-refs.lock`) the next operation can trip over | (i) — no Cairn document mentions it |
| `gpg` / `gpg-agent` / pinentry | git, under `commit.gpgSign` / `tag.gpgSign` (i) | a credential-like prompt outside the askpass path; needs `GNUPGHOME` and a display (#18 open) | #18, CRED "Not here" (GPG signing) |
| The user's own `git`, IDE, file watcher | not Cairn | lock contention Cairn can report but not prevent ("a listing cannot tell a stranded lock from a live one") | CRED, #19 |

---

## Synthesis

### (a) The invocation shapes the manager must support

1. **Probe.** Tiny, run to completion, stdout captured, not cancellable: `git
   --version` (E1). Today's `run()`.
2. **Short captured read, `-z` stdout, epoch-cancellable.** No token, no locks
   (`GIT_OPTIONAL_LOCKS=0`), parsed output: the changes query (D1, the packet's
   first consumer, about 8–83 ms and up to 5.5 MB of records), status if O2
   picks git (S1, 29.5 ms clean), working-tree lists (D2), dry-runs that feed
   a prompt (P7, N5), config origins (M16). Neither runner fits today: `run()`
   captures but cannot be killed and buffers without limit; `stream()` can be
   killed but discards stdout (SURV).
3. **Long streaming read.** Records consumed as they arrive, back-pressure,
   epoch cancel that stops the process: candidates only (M9 blame, M10
   `--follow`, M11 pickaxe). No document commits to one yet, but the parity
   rule and option E make them likely.
4. **Short local mutation, optionally with stdin.** Exclusive on the index
   or refs, stderr captured for the error, effectively not cancellable once
   running: add/restore/apply (P1–P7, with a patch, a message or a NUL
   pathspec list on stdin), branch/tag (B1–B6), stash (P10–P12), reset (M7),
   config and remotes (N3, N4, M15). stdin is new: both runner paths pin it to
   null and a test enforces it (SURV gap 2; RM 5).
5. **Hooked mutation.** Runs the user's programs for an unbounded time: commit
   and amend (P8, P9), push (`pre-push`), merge, rebase, checkout. Needs
   streamed stderr (hook output is the progress), a user cancel that reaches
   the hook's process group, an editor that can never open, and possibly a
   GPG pinentry.
6. **Network operation.** Fetch (built), push, pull, prune, clone, submodule
   update: progress on stderr (coalesced, #25), an askpass token per
   invocation, user cancel `SIGTERM`→`SIGKILL` with stranded locks reported,
   and no timeout today.
7. **Sequencer mutation that can stop.** Merge, rebase, cherry-pick, revert,
   stash apply/pop: exit 1 with conflicts is a *state*, not a failure, and is
   followed by continue / skip / abort. The outcome vocabulary needs "stopped",
   read from the repository's state rather than from stderr, which is never
   parsed (OPS).
8. **Interactive through a helper program.** Interactive rebase (M4):
   `GIT_SEQUENCE_EDITOR` and `GIT_EDITOR` name a Cairn program that round-trips
   to the window, the same shape as askpass, with git blocked for minutes on
   the UI.
9. **Repository-less.** Clone and init (N7, M14): no repository to key,
   serialize or invalidate against; the working directory is a parent.
10. **Not git at all.** The system opener (N8), terminal, editor, external
    diff and merge tools (M17). Fire-and-forget or long-lived GUI processes.
    The terminal-prompt guard allows `Command` only in
    `ops/environment.rs`, so either the manager hosts them or a sibling
    construction point is decided.

**No long-lived batch process appears in any document.** Nothing mentions
`git cat-file --batch`, `--batch-check` or a persistent git child. The one
remark nearby is BASE's "A long-lived worker does not pay [the 2.4 ms floor]",
which is about gix in process. The only long-lived child processes in the
designs are gix's own filter `process` clients (section 9).

### (b) Concurrency and ordering rules the consumers imply

1. **Reads take no locks.** Every read invocation runs with
   `GIT_OPTIONAL_LOCKS=0`, or `git status` may refresh and rewrite the index
   (BASE method; SURV gap 12). The variable is on neither the roster nor
   `ALWAYS` today, so read and write environments differ.
2. **Reads run beside anything, and a write makes them stale.** A read
   computed during a write can see a transient state. When a write finishes,
   its `Invalidated` flags decide which lanes re-run (OPS). Today the worker
   decides by comparing `ref_tips`, not by reading the declaration (#25 item 5).
3. **Index writers are mutually exclusive per worktree.** That is every
   staging verb, commit, checkout, reset, stash and sequencer verb. git does
   not wait for `index.lock`: the second writer fails with "Unable to create
   … index.lock" (i). So the manager must serialize. Relying on git turns
   contention into user-visible errors.
4. **Ref writers contend per ref and on `packed-refs.lock`**, which lives in
   the common dir and is shared by every worktree: fetch, push, commit,
   branch, tag, stash and auto-gc's pack-refs (i).
5. **Today: one operation at a time per repository, FIFO**, on one operations
   thread, and a second fetch is silently dropped (SURV). Under that rule a
   two-minute push or a slow `pre-commit` blocks every stage click behind it.
   The consumers above split naturally into network operations (remote-side
   work, which touch only tracking refs locally) and local index/worktree
   operations. Whether those two may overlap is the central scheduling
   decision.
6. **A repository mid-sequencer accepts only that sequencer's verbs.** While
   MERGING or REBASING, a checkout, another rebase or a stash is wrong. git
   refuses most of them itself (i), but the manager or the engine should gate
   on the repository's state before spawning anything.
7. **Hooks mutate under a running commit.** A pre-commit hook can restage
   files or stash (FORK; lint-staged-style hooks (i)), so Cairn must not
   schedule its own index work, or trust an index read, while a commit is
   inside its hooks.
8. **Write, then read in one request: reopen the handle.** gix cannot see an
   index rewritten inside the same timestamp tick (OPS, `index` flag), so
   staging followed immediately by a status needs a fresh handle.
9. **Worktrees decide the exclusion key.** The index and HEAD are per
   worktree; refs, objects and config are per common dir. So "per repository"
   has to say which.
10. **Track and reap on shutdown.** Closing a repository or the window must
    kill and reap every child, process groups included (#27; SURV gaps 7 and
    9). A `Running` dropped without `finish` is neither killed nor reaped today.
11. **External git processes are reported, not prevented.** The user's
    terminal, an IDE and a detached auto-gc can all hold locks. Cairn can name
    them but cannot tell them from its own strays (CRED; #19).

### (c) Decisions already locked that bind the manager

- **Option E amends D1.** "the changes query … comes from `git diff-tree -M`
  always; gix keeps content diffs, history and the model … This amends D1. A
  process manager for spawning git is to be designed and built first, as its
  own packet" (`docs/research/diff-engine/rename-parity-spike.md`, "What was
  decided from it"; DPROG 2026-09-30). The DPROG entry adds that phase 03's
  working-tree reads and packet 5's staging build on the manager too.
- **D1 otherwise stands.** "The `git` CLI never runs on a read path" (ENG)
  is now false for the changes query. ENG and the root CLAUDE.md still say
  it, so the manager packet owns writing the amendment down. Writes go to git
  "because of hooks, not coverage" (ENG).
- **The environment invariant.** Every git subprocess runs with
  `GitEnvironment`: `GIT_TERMINAL_PROMPT=0`, `SSH_ASKPASS_REQUIRE=force`, and
  `GIT_ASKPASS`/`SSH_ASKPASS` naming Cairn's helper. `GitEnvironment::command`
  is "the only place a `std::process::Command` is built" (root CLAUDE.md;
  twin `every_git_invocation_disables_the_terminal_prompt`).
- **L5, built never inherited.** "every passed variable is a deliberate entry
  with its reason beside it in `environment.rs`" (CRED, L5). Locale variables
  are inherited "because git's stderr is shown verbatim". That pulls against
  `LC_ALL=C` for parsed reads.
- **D2 and #22: Cairn's helper is always the askpass**, even over a user-set
  `GIT_ASKPASS`/`SSH_ASKPASS`/`core.askPass` (CREDD).
- **The askpass token is per invocation.** "The token is scoped to one git
  invocation, not one ask" (CRED, "Taken inside the phases").
- **Only `ops/` mutates, and the destructive verbs take `Confirmed` by
  value.** "Nothing outside `ops` can run a raw verb: the public surface is
  named operations" (OPS). The runner is `pub(crate)`, so that seal is
  crate-level, and the `spawns_git` matcher cannot see `git.command()` from
  `diff/` (SURV, a guard gap).
- **Output policy.** "`-z` for anything that lists paths … `--porcelain=v2`
  for status … Standard error is prose for a person: it travels into the error
  verbatim and is shown, never matched on" (OPS).
- **Errors.** `GitFailed` carries the arguments, status and stderr, so "git
  failed" is never the whole message. An outcome a caller must tell apart
  comes "from a channel of its own" (OPS).
- **Cancel.** `SIGTERM`, then `SIGKILL` after `TERMINATION_GRACE` = 2 s; never
  signal a reaped pid; list stranded `*.lock` files after the reap (#19, CRED).
  `nix` is the accepted dependency for it.
- **D3 and L8: epochs per lane, and an epoch is the cancel.** "The epoch is the
  cancel signal itself … superseding a query stops its walk rather than
  discarding its answer"; "An operation such as fetch carries no epoch … an
  operation is cancelled by killing its process" (CONC). The lanes are
  history, changes and file diff, and a changes query also supersedes the
  file diff (DBS L8). The UI thread never waits (root CLAUDE.md twin).
- **The invalidation contract.** Every mutation declares `refs`, `index`,
  `objects` and `working_tree` on its `Performed`, and the worker honours them
  (OPS, ENG).
- **The version floor.** "Minimum git is 2.30 … a support policy and so the
  user's to raise" (CRED, L9). That rules out `fetch --porcelain` (2.41) and
  `worktree list -z` (2.36 (i)) without a decision.
- **The fetch policy (#17).** Honour `fetch.prune`, always pass
  `--no-prune-tags`, and refuse mirror or `refs/heads/*` refspecs before
  spawning.
- **The editor (#18, proposed direction, not yet landed).** "`GIT_EDITOR`:
  pin to fail closed in `ALWAYS` the moment a verb that can open an editor
  lands, in that verb's change".
- **The filter-driver residual (DBS L6).** The driver runs with Cairn's
  inherited environment, and the environment invariant "stays scoped to `git`
  processes".
- **Dependencies are the user's call.** There is no logging crate on
  `cairn-git`'s allowlist (SURV gap 11; root CLAUDE.md).
- **User standing rules (auto-memory).** "diverging from what git shows is a
  critical bug". "new git spawning waits on a process-manager packet".

### (d) Open questions the manager must answer

1. **Where it lives and what the guard sees.** Either amend `ops/`'s
   "mutations" charter or add a read-invocation module beside it. Either way,
   give the guard a read/write distinction and close the `spawns_git` gap that
   lets `diff/` reach the runner unseen (SURV gaps 1, "Invariants").
2. **One `GitBinary` per application or per repository?** Today it is
   discovered on every open and held only by the operations thread. The diff
   thread needs one (SURV gap 15).
3. **The environment for reads.** `GIT_OPTIONAL_LOCKS=0` always. `LC_ALL=C`
   for parsed output versus the inherited locale for stderr the user reads.
   Are these per-shape environments, and how does the terminal-prompt twin
   pin them?
4. **The exclusion model.** One write per repository, per worktree, or
   separate network and local lanes? When a write is busy, does the next one
   queue, get refused, or coalesce? What does the UI show while it is queued?
5. **What each shape can cancel, and when.** Hooks can be cancelled; a ref
   update or index rewrite cannot be safely. Kill by process group. Kill on
   `Drop`. Kill reads on epoch supersede, or let a 10 ms read finish and
   discard it?
6. **The epoch-to-kill bridge.** Poll a `&impl Cancel` in `finish`'s 20 ms
   loop (SURV), or push from the epoch counter?
7. **Bounds.** Stream or cap stdout (S1 is 5.5 MB; a dirty status and a
   pickaxe log are unbounded). Cap stderr (#25 item 4). Coalesce progress to
   one update per frame (#25 item 1).
8. **Timeouts and stall detection.** "nothing times a fetch out" (CRED). A hung
   hook or a stalled network today ends only when the user cancels.
9. **The editor protocol.** Fail-closed `GIT_EDITOR` for now (#18), and a
   Cairn editor/sequence-editor helper for interactive rebase later. Does that
   reuse the askpass channel and token?
10. **Signing.** Do `GNUPGHOME` and `DISPLAY`/`WAYLAND_DISPLAY` reach git (#18
    open)? What does a commit under `commit.gpgSign` do when pinentry has no
    display?
11. **Non-git spawns.** Does the manager host the system opener, terminal,
    editor and merge tool, or does a sibling construction point get its own
    guard row?
12. **Repository-less invocations.** How are clone and init keyed and tracked?
13. **The registry and shutdown.** What happens on repository close and on
    window close (#27)? Should `-c gc.autoDetach=false` or
    `-c maintenance.auto=false` (i) keep auto-gc inside the tracked process,
    or should it run detached?
14. **Outcomes that do not live in exit code plus stderr.** Option E's
    rename-limit warning (R2.2 "the answer says so"); `cannot lock ref`
    naming a lock (#19 remainder); "patch does not apply" against "no
    longer applies because the file changed"; conflicts as a stopped state
    (exit 1). Which of these are read from git's state rather than its prose?
15. **A read's error vocabulary.** It needs to be distinct from
    `GitCancelled { stranded_locks }`, which is mutation language (SURV gap 14).
16. **Plumbing and config.** `diff-tree` is plumbing and may not read
    porcelain config such as `diff.renames` (i, unverified). Who turns the
    user's `diff.renames` / `diff.renameLimit` into `-M` / `-C` / `-l`, and
    through what `-c` policy (DBS L4: "Rename detection follows the user's
    config")?
17. **stdin mechanics.** A patch can be tens of MB at the Load Diff ceiling.
    Write from a separate thread, or sequentially before reading? The pipe
    deadlock rule.
18. **How far the manager reaches.** Does O2 (status from git) and the phase
    03 working-tree lists (D2) put the most frequent query in Cairn on the
    manager? Do parity pressures move blame, `--follow` and pickaxe there too
    (M9–M11)?
19. **Observability.** Per-invocation timing and logging for the operation
    log and for diagnosis, with no logging crate allowed today.
20. **Portability.** The kill path is Unix-only (`nix` is unconditional,
    SURV gap 13), which is acceptable under D5. Whether macOS signals and
    process groups behave the same is unverified.
