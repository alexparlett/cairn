# Brainstorm — process-manager

Locked decisions and rejected alternatives. Historical record: never retro-edited.
Evidence for every decision is under `docs/research/process-manager/`, and the
packet was commissioned by `docs/research/diff-engine/git-process-survey.md` and
`rename-parity-spike.md`.

## How this was decided

`diff-engine` paused on 2026-09-30 when gix's rename detection disagreed with
git's on a large rollup (231 pairs against 2,774). The user classed that as a
critical bug, chose to take the changes query from `git diff-tree -M` (that
packet's option E), and asked for "a proper process manager designed and built
around spawning git processes" first, as its own packet. The survey that
commissioned it found a careful single-invocation runner built for fetch, not a
manager.

Four recon records were gathered on 2026-10-02:

- `runner-and-worker-as-built.md` — the runner, worker and guards, verified
  against current code. It confirmed the survey's guard gap, which turned out
  wider than reported. Any `cairn-git` module can run any verb through four
  routes, and nothing guards a gitoxide write outside `ops/`.
- `consumer-invocations.md` — every `git` invocation the daily loop and the
  second lap need: about 55 rows, ten shapes.
- `precedent-study.md` — GitHub Desktop and dugite, VS Code, jj, GitButler,
  lazygit, Magit, and what is public about Fork and Sublime Merge.
- `platform-and-git-behaviour.md` — std 1.97.1, nix 0.31.3 and git 2.56.0,
  read in source and measured where it mattered.

Decisions were presented with mechanism, cost, what each forecloses and the
evidence. The user locked the four structural ones (L1-L4) by choosing the
recommended option for each. The eleven smaller ones (L5-L15) were presented as
defaults that stand unless objected to, and none was.

## Locked 2026-10-02

**L1. Scope: the manager and every I/O shape the daily loop needs, with no new
verb.** The runner gains captured-and-cancellable stdout, stdin,
process-group kill, bounded stderr, a registry with kill-all, and read and write
environments. Fetch and the probe move onto it and the old `run` and `stream` are
deleted. Each shape is proven against real `git` in tests. The `diff-tree`
parser and the changes-query rework stay in `diff-engine`; stdin's first verb
stays in `staging-and-commit`.
Rejected: the read shape only — smaller, but packets 5 and 6 would each reopen
the runner, which is how the fetch-only runner came to need this packet.
Rejected: also building the editor helper for interactive rebase, which has no
consumer before the second lap.
Evidence: `consumer-invocations.md` synthesis (a): shapes 2, 4, 5 and 6 are
needed by D7, shapes 7, 8 and 9 only after it.

**L2. Placement: a crate-private `process/` module, a `reads/` module, and a
`WriteAuthority` token.** `process/` is the only place a process is built. An
invocation is typed read or write when it is built, and a write needs a
`WriteAuthority` whose constructor is private to `ops/` — a token, because
`pub(in crate::ops)` cannot be written from a sibling module. Only `ops/` and
`reads/` may call the runner, and a new twin also closes the existing gap: no
gitoxide mutation API outside `ops/`.
Rejected: keeping everything in `ops/`, rechartered as "every git process" —
simpler, but it blurs the line the operation log and the `Confirmed` seal stand
on. Rejected: a separate `cairn-process` crate — the seal would come from the
allowlist, but the environment twin would move crates for nothing the token does
not already buy.
Evidence: `runner-and-worker-as-built.md` section 3 (the three unguarded routes,
the `spawns_git` matcher's reach, the untwinned gitoxide half).

**L3. Concurrency: reads run free; writes go through two lanes, network and
local.** A read takes no lock (gix in process; `git` with `GIT_OPTIONAL_LOCKS=0`)
and is cancelled by its lane's epoch. The network lane holds fetch, push and the
transfer half of pull; the local lane holds every index, worktree and local-ref
write, one at a time. The lanes overlap, because where they meet git's ref locks
retry, and only the local lane writes the index, which git never waits on.
A user's operation queues while its lane is busy and is shown queued. A duplicate
network operation is refused with a reason, not dropped as today. Background work
is skipped while its lane is busy. Lanes are keyed per repository (common
directory); a per-worktree split is left open for the worktrees packet. This
packet builds the read path, the network lane and the refusal; the local lane's
thread lands with packet 5's first verb, so nothing is built for a hypothetical
consumer.
Rejected: one write lane for everything — it cannot race, but a slow push or
`pre-commit` stalls staging. Rejected: a read/write lock (GitButler's model),
which makes reads wait on writes they do not conflict with.
Evidence: `platform-and-git-behaviour.md` C6 (no retry on `index.lock`; ref
timeouts of 100 ms and 1 s); `consumer-invocations.md` synthesis (b);
`precedent-study.md` sections 1, 2, 4 and 5 and lessons 6 and 10 (Fork #1633's
fetch race; background work fails closed rather than prompting).

**L4. Kill: process group, `SIGTERM` then `SIGKILL`, cancel by poll, handle or
drop.** Every `git` starts with `CommandExt::process_group(0)`: safe, stable
since 1.64, works on macOS. Ending it is `killpg` `SIGTERM`, then `SIGKILL` after
2 s. nix's `signal` feature already covers it, so no new dependency. A read polls
its query's `&impl Cancel` in the wait loop; an operation hands out a
non-blocking kill handle; a drop kills and reaps on a reaper thread.
Rejected: a pid-only kill with a pipe timeout. That is the mechanism measured
leaving an orphan holding stderr for 3.7 s, and the source of the uncancelled
hang the audit found.
A cancel that loses the race to a clean exit is success, as fetch's is today.
Stated residuals:
- a cancelled operation that started `credential-cache--daemon` ends the user's
  cache (measured: the daemon stays in git's group);
- a child that reads `/dev/tty` while Cairn was launched from a terminal is
  stopped rather than failing;
- detached auto-gc, the fsmonitor daemon and an ssh `ControlPersist` master
  escape the group;
- children outlive a crash of Cairn, since kill-on-parent-death needs `unsafe`;
- a narrow race in which a group id is reused between the liveness check and the
  signal (evidence B8: not closable without pidfds).

Evidence: `platform-and-git-behaviour.md` A1, A9, B, C1, C2 (all measured);
`precedent-study.md` lessons 1 and 2.

**L5. One process per invocation.** No long-lived `cat-file --batch` child. No
consumer needs one, and `--batch-command` (2.36) and `-Z` (2.42) are above the
2.30 floor. Evidence: `platform-and-git-behaviour.md` C12; `precedent-study.md`
(no GUI client keeps one).

**L6. One thread per pipe used.** stdout reader, stderr reader and stdin writer,
std only. Rejected: a single thread polling the pipes, which needs nix's `poll`
and `fs` features turned on — no new crates, but a dependency-policy change and
more code for partial writes. Evidence: `platform-and-git-behaviour.md` A6 and
D2 (the deadlock measured, and possible after 512 bytes on macOS).

**L7. Output bounds.** stdout goes to the caller as bytes or `-z` records as it
arrives; a collect states a per-verb ceiling and errors past it. stderr keeps a
256 KiB tail (Desktop's figure), and progress lines are forwarded, then
dropped. This narrows #25 without closing it: coalescing progress to a frame
stays there. Evidence: `precedent-study.md` lesson 3;
`consumer-invocations.md` (S1 is 5.5 MB of records).

**L8. The locale stays inherited; nothing parses translated text.** Only
untranslated formats are parsed (`-z`, `--raw`, porcelain v2), and failure is
classified by exit status and repository state. There is no automatic retry on
`index.lock`: Cairn names the lock and never deletes it.
Rejected: `LC_MESSAGES=C` (jj) or `LC_ALL=C` (GitButler), which would put every
error the user reads in English to buy parsing nothing needs.
Evidence: `platform-and-git-behaviour.md` C5 (measured under German) and C6;
`precedent-study.md` lesson 5 (a retry must not re-run a confirmed mutation).

**L9. Environment profiles.** A read adds `GIT_OPTIONAL_LOCKS=0` and carries no
askpass token. Only `status` honours that variable, so a read runs plumbing or
`status`, never porcelain `diff` or `describe --dirty`. Every invocation adds `GIT_EDITOR=false` and
`GIT_SEQUENCE_EDITOR=false`, which settles #18's editor half. `false` rather
than `:`, because `:` silently accepts a proposed merge message. Both are needed
because `sequence.editor` outranks `GIT_EDITOR`. Auto-maintenance stays git's,
for parity.
Evidence: `platform-and-git-behaviour.md` C2, C3, C9.

**L10. Discovery once per application.** `GitBinary` is found at startup, as
credential-prompts R1.5 first said, and cloned to each thread that runs git. The
as-built discovers it on every repository open. Evidence:
`runner-and-worker-as-built.md` section 2.

**L11. Registry and shutdown.** Each repository tracks its running invocations.
Closing it ends them all and waits a bounded time for the reaps. This picks up
#27. Evidence: `runner-and-worker-as-built.md` (threads are detached and never
joined; on window close the handle's fate is OPEN).

**L12. No timeouts.** Hooks and pushes are legitimately long. The window shows
elapsed time and a cancel button; reads are cancelled by their epoch. Evidence:
`precedent-study.md` lesson 9.

**L13. The command log as data.** Each entry records the arguments, working
directory, start, duration, exit, whether it was cancelled, and the stderr tail.
The log is bounded, in memory, and never records the environment or a `Secret`.
An entry is a `cairn-model` type. Fork's visible panel is a follow-up issue, not
this packet. There is no logging crate. Evidence: `precedent-study.md` lesson 8
and its comparison table (VS Code, lazygit and Magit show one, and Fork does).

**L14. The record.** This packet rewrites D1 in `docs/design/engine.md`: a read
may run git where gix diverges from git, with each such read named in `reads/`.
It adds `docs/design/processes.md` and the write lanes in `concurrency.md`, and
corrects the root `CLAUDE.md`'s "Reads never spawn a process". The design docs
land with the planning PR; `CLAUDE.md` lands with the code that makes it true.

**L15. Unix only, Linux and macOS**, as today. `nix` stays unconditional.

## Left to other packets, deliberately

- **Whether `diff-engine`'s phase 03 working-tree reads move to git.** Its
  progress log says they "build on" this packet, but its R3 still has gix doing
  them. `diff-engine` decides on resume, against this packet's runner.
- **The rename-limit signal.** `diff-engine` R2.2 says a changes answer cut short
  by `diff.renameLimit` says so. `git diff-tree` reports that only as a translated
  stderr warning, and L8 forbids parsing it. `diff-engine` must find a signal
  that is not prose, or take the conflict to the user.
- **Who turns `diff.renames` and `diff.renameLimit` into `-M`, `-C` and `-l` for
  `diff-tree`**, which is plumbing and may not read porcelain config (unverified).
  The question belongs to `diff-engine`'s changes query.
- **Index lock contention with the user's own git**, beyond naming the lock:
  `staging-and-commit`, when the local lane exists.
- **Signing.** Whether `GNUPGHOME` and the display variables reach git, and what a
  commit under `commit.gpgSign` does without a pinentry (#18's other half):
  `staging-and-commit`.
- **Non-git spawns** — the system opener, a terminal, a merge tool. Whether
  `process/` hosts them or a sibling construction point gets its own guard row is
  decided by the first packet that needs one.
