# The git verbs staging-and-commit would run, verified

Research for the `staging-and-commit` packet, recorded 2026-10-07.
Commissioned by: staging-and-commit planning. Evidence record, saved in full;
historical, never retro-edited. It decides nothing: it records what each `git`
verb the packet could run actually does, under Cairn's environment, at the floor
and at the newest git available.

## Method

- **Git versions tested:** 2.30.9 and 2.32.7 (the floor builds in
  `~/.cache/cairn/git-floor/`, built by `scripts/git-floor.sh`) and the system git,
  **2.56.0**. Where a behaviour was the same on 2.30.9 and 2.56.0 it is reported
  once; 2.32.7 was run only where a version gate sits between the two.
- **Environment:** every experiment ran with the variables Cairn's `ALWAYS` table
  sets — `GIT_TERMINAL_PROMPT=0`, `GIT_EDITOR=false`, `GIT_SEQUENCE_EDITOR=false`,
  `SSH_ASKPASS_REQUIRE=force`, `GIT_ASKPASS` and `SSH_ASKPASS` set to `/bin/false`
  (a helper that refuses) unless a test says otherwise — plus
  `GIT_CONFIG_NOSYSTEM=1` and a scratch `HOME` holding only `user.name`,
  `user.email` and `init.defaultBranch=main`. Writes were run WITHOUT
  `GIT_OPTIONAL_LOCKS=0`, as a write invocation is.
- **Where:** fresh repositories built by real git under the session scratchpad
  (`.../scratchpad/git-verbs/`), one shell script per section (`s1.sh` … `s11.sh`,
  kept there beside this record's transcripts). Kills used a small Python driver
  that starts git with `process_group=0` — the same `CommandExt::process_group(0)`
  Cairn's runner uses (`crates/cairn-git/src/process/cli.rs`) — and SIGKILLs the
  group. No experiment touched the bench repository or this checkout.
- **Docs:** the local 2.56 man pages (`man git-commit`, `git-apply`, `git-clean`,
  `git-stash`, `git-config`, `git`) and the release notes at
  `github.com/git/git/blob/master/Documentation/RelNotes/<version>.adoc` for
  version gates.
- **Tags:** **VERIFIED** (by experiment here; a transcript excerpt follows),
  **DOCUMENTED** (cited), **UNVERIFIED** (reasoned or remembered, not checked).
  Exit codes are reported as observed; Cairn classifies failure by exit code and
  repository state, never by stderr (`docs/design/processes.md`), and several exit
  codes below differ between 2.30 and 2.56, which matters for that rule.

## Headlines

- **The patch Cairn feeds `git apply` must be in git's form (the clean, LF,
  index form) for BOTH `--cached` and worktree application.** `apply --cached`
  runs no filter and no eol conversion: it patches the index blob's bytes. Worktree
  `apply` (no `--cached`) cleans the file on read and smudges on write, so a
  git-form patch applies to a CRLF or filtered worktree file and the file keeps its
  worktree form. A patch in worktree form (CRLF, smudged) is refused by both.
  VERIFIED, 2.30 and 2.56.
- **The user's `apply.whitespace` config silently changes what is staged.**
  `apply.whitespace=fix` strips trailing whitespace from the staged lines (index
  then differs from the worktree the user saw); `=error` makes staging fail with
  exit 128. `--whitespace=nowarn` on the command line overrides both. VERIFIED.
- **`git stash push` is not atomic, but it stores the entry first.** SIGKILLed in
  its `reset --hard` phase it leaves `refs/stash` written, the worktree half-reset
  (a file deleted mid-smudge) and a stale `index.lock`; every change is in the
  stash. It writes the index six times in one push. VERIFIED.
- **`git stash drop` will not take an oid** ("is not a stash reference"); only
  `stash@{n}`, so dropping by position races a concurrent stash. A dropped stash
  is in NO reflog afterwards; only `git fsck` finds it (as a dangling commit) and
  `git stash store` puts it back. **Stash entries never expire by default** (git
  special-cases `refs/stash`). VERIFIED.
- **`git stash push` fails outright when the index holds an intent-to-add entry**
  ("Entry 'n' not uptodate. Cannot merge."), on 2.30 and 2.56 alike. VERIFIED.
  Partial staging of a new file therefore must not leave `add -N` entries lying
  around, or stash breaks.
- **"Nothing to stash" exits 0** (`No local changes to save`), so whether a push
  stored anything must be read from `refs/stash`, not the exit code. VERIFIED.
- **A hook that reads `/dev/tty` hangs forever under Cairn's process group when
  Cairn was started from a terminal**: the hook is in a background process group
  of the controlling terminal and is stopped by SIGTTIN (state `T`). Without a
  controlling terminal (`setsid`, a desktop launch) the open fails with ENXIO and
  the hook carries on. VERIFIED (2.56).
- **SSH commit signing with a passphrase-protected key goes through
  `SSH_ASKPASS`** — so a commit is a write that needs an askpass token if signing
  is to work. `gpg.format=ssh` makes 2.30 and 2.32 fail every commit with
  `fatal: bad config variable 'gpg.format'`. VERIFIED.
- **`i18n.commitEncoding` set to anything but UTF-8 turns a UTF-8 message from
  Cairn into mojibake**: git stores the bytes as given under an `encoding` header
  and `git log` re-encodes them. VERIFIED.
- **Stage, unstage and file-level discard all exist on the floor**, including
  `--pathspec-from-file=- --pathspec-file-nul` for `add`, `reset`, `restore`,
  `checkout`, `rm` (2.26) and `stash` (2.26) — but **not `clean`** (unknown option,
  2.30 and 2.56). `git reset -q -- <path>` unstages on an unborn branch;
  `git restore --staged` does not (`fatal: could not resolve HEAD`). VERIFIED.

## 1. Partial staging: `git apply --cached`

**Command line.** `git apply --cached --whitespace=nowarn [--recount] [--unidiff-zero] [-C<n>] [--check]`
with the patch on stdin (no path argument reads stdin). Unstage is the same with
`-R`. **Stdin:** the patch. **Stdout:** nothing on success. **Stderr:** diagnostics.

**Exit codes (VERIFIED, both versions):**

| Outcome | Exit | Stderr (2.56 spelling) |
| --- | --- | --- |
| Applied | 0 | — |
| Preimage does not match the index | 1 | `error: patch failed: f.txt:1` / `error: f.txt: patch does not apply` |
| Path unmerged (conflict stages) | 1 | `error: a.txt: does not exist in index` |
| Binary patch without `--binary` | 1 | `error: cannot apply binary patch to 'b.bin' without full index line` |
| Hunk header counts wrong (no `--recount`) | 128 | `error: corrupt patch at bad.patch:11` (2.30: `at line 11`) |
| Whitespace error under `--whitespace=error` / `apply.whitespace=error` | 128 | `error: 1 line adds whitespace errors.` |
| `index.lock` held | 128 | `fatal: Unable to create '.../index.lock': File exists.` |
| Empty input | 128 | 2.30: `error: unrecognized input`; 2.56: `No valid patches in input (allow with "--allow-empty")` |

**What it writes.** The index only: new blob objects for the patched entries, then
the index via `index.lock` and rename. No ref, no reflog, no worktree. Runs the
`post-index-change` hook (args `0 0`) and no other (VERIFIED, `s11.sh`). Takes
`index.lock` (VERIFIED: fails under a held lock). After it, plumbing
`git diff-files` reports a file whose whole change was staged as modified (its
stat data is not refreshed), where `git status` reports it correctly:

```
-- stat-dirty after apply --cached of the whole change
sp ace                       <- git diff-files --name-only
1 M. N..                     <- git status --porcelain=v2
```

(VERIFIED; matters only to a reader that trusts stat data — Cairn's status is
`git status`.)

**What it can destroy.** Nothing irrecoverable: the previous index state of the
file is either `HEAD`'s blob or a blob still in the object store until pruned; a
wrong apply is undone by `-R` of the same patch. Under `apply.whitespace=fix` it
records content the user never had (below), which is a silent corruption of
intent rather than of data.

### Line-level and hunk-level patches (VERIFIED)

- One hunk of a two-hunk `git diff` applies alone; `--check` checks without
  writing; `-R` of `git diff --cached` unstages.
- A hand-built line selection (keep `2`, add `two` — "stage only the added line")
  applies with correct counts; with wrong counts it is `corrupt patch` (128) and
  `--recount` makes it apply. The `index` line in the header is not needed for a
  text patch.
- Re-applying an already-staged hunk fails with exit 1 (`patch does not apply`),
  so a duplicated click is refused, not doubled.
- **Context drift** (the index changed under the patch's context): plain apply
  fails; `-C1` applies (`Context reduced to (1/1) to apply fragment at 4`).
  DOCUMENTED (`git-apply -C<n>`): "By default no context is ever ignored."
- **Overlapping hunks** in one patch fail; `--allow-overlap` accepts them.
- **`-U0` patches** fail without `--unidiff-zero` and apply with it.
- **No newline at EOF:** `\ No newline at end of file` markers apply correctly
  (`x\ny` → `x\ny\nz\n`).
- **Odd names:** git's C-quoted headers (`"a/t\tab"`, `"a/\303\274n\303\257"`) and
  the unquoted UTF-8 form `core.quotePath=false` prints both apply. A path with a
  space is unquoted (`diff --git a/sp ace b/sp ace`) and applies.

### Conversions: CRLF, `core.autocrlf`, clean filters (VERIFIED)

With `core.autocrlf=true` and a CRLF worktree file, the index blob is LF and
`git diff` prints LF (`grep -c $'\r' p.patch` → `0`). That patch applies with
`--cached` and the index stays LF. The same hunk re-written with CRLF line ends
fails (`patch does not apply`, exit 1). With a rot13 clean/smudge filter on `*.r`,
the index holds the cleaned bytes (`nycun`, `orgn`), `git diff` prints the cleaned
form, the cleaned-form patch applies with `--cached`, and **no filter process
ran** (a logging filter wrote nothing). So `--cached` patches exactly the index
blob's bytes; the patch must be built from the index blob and git's form of the
worktree file — which is what Cairn's `reads::working_tree_patch` already reads.

### New files, deletions, modes, binary, intent-to-add (VERIFIED)

- **Untracked file, whole:** `git diff --no-index -- /dev/null new.txt` output
  (`new file mode 100644`) applies with `--cached` → status `A.`.
- **Untracked file, partial:** a `new file mode` patch adding only line 1 →
  status `AM` (index has one line, worktree three). No `add -N` needed.
- **Intent-to-add entry:** both a `new file mode` patch and a plain modification
  patch (`@@ -0,0 +1 @@`) apply to an ITA entry → `AM`. (But see §7: an ITA entry
  breaks `git stash`.)
- **Deletion:** the `deleted file mode` patch from `git diff -- gone.txt` applies →
  `D.`; `-R` restores the entry. A partial deletion (remove two of four lines of a
  file deleted in the worktree) applies → `MD`.
- **Mode change only:** `old mode 100644 / new mode 100755` applies → index entry
  `100755`.
- **Binary:** `Binary files a/b.bin and b/b.bin differ` is refused (exit 1); a
  `git diff --binary` patch applies. Hunk-level staging of a binary is not a thing;
  file-level `git add` is the route.
- **Unmerged path:** refused (exit 1, `does not exist in index`) — resolving a
  conflict is `git add`, not apply.

### Whitespace config (VERIFIED)

```
$ git -c apply.whitespace=fix apply --cached ws.patch      # patch adds "2   "
warning: 1 line applied after fixing whitespace errors.
exit=0
0000000   2  \n                                           # index: trailing spaces gone
-2
+2                                                        # worktree still differs
$ git -c apply.whitespace=fix apply --cached --whitespace=nowarn ws.patch
0000000   2              \n                               # staged as the user had it
$ git -c apply.whitespace=error apply --cached --whitespace=nowarn ws.patch
exit=0
```

`apply.ignoreWhitespace` (context matching that ignores whitespace) is a second
config the user may set; `--no-ignore-whitespace` exists on 2.56 (`git apply -h`
shows `--[no-]ignore-whitespace`) but **not on 2.30** (its help lists only
`--ignore-whitespace`), so on the floor only `-c apply.ignoreWhitespace=false`
would override it. Its effect on a staged hunk is UNVERIFIED.

### Floor status

| Flag | 2.30 | Gate |
| --- | --- | --- |
| `--cached`, `-R`, `--check`, `--recount`, `-C`, `--unidiff-zero`, `--allow-overlap`, `--whitespace=`, `--binary` patches | yes | — (VERIFIED) |
| `--allow-empty` | **no** (`unknown option`, 2.30 and 2.32) | 2.35, DOCUMENTED: "'git apply' has been taught to ignore a message without a patch with the '--allow-empty' option" |
| `--3way` with `--cached` | **no** (`--cached and --3way cannot be used together`) | 2.32, DOCUMENTED: "'git apply' now takes '--3way' and '--cached' at the same time" (VERIFIED working on 2.32.7) |
| `--no-ignore-whitespace` | no | after 2.30 (exact version UNVERIFIED) |

## 2. Discarding hunks and lines

**From the worktree:** `git apply -R --whitespace=nowarn` (no `--cached`), git-form
patch on stdin.

- **Filters and eol run** (VERIFIED): with a logging rot13 filter, a worktree
  `apply -R` logged `CLEAN x.r` then `SMUDGE x.r`; with `core.autocrlf=true` the
  discarded hunk was reverted and all 20 lines kept CRLF. A worktree-form
  (smudged) patch is refused (exit 1). So the same git-form patch Cairn shows
  serves both the index and the worktree.
- **Takes no `index.lock`** and writes no index (VERIFIED: succeeds while
  `index.lock` is held). Runs no hook (VERIFIED, `s11.sh`).
- **Stale preimage** (the user typed into the file after the diff was read):
  fails, exit 1, nothing written. `-C0` would force it — never appropriate for a
  discard. VERIFIED.
- **What it destroys:** the discarded lines are in no object (worktree edits were
  never hashed) — **unrecoverable** by git. This is the operation that needs
  `Confirmed`.

`git checkout -p` / `git restore -p` / `git add -p` are interactive (they read
choices from stdin per hunk) and are not usable as a programmatic verb.
DOCUMENTED (`git-checkout --patch`: "interactively select hunks").

**Discarding a staged hunk** (out of index and worktree):

- `git apply --index -R` does both at once but requires the worktree file to match
  the index: with any other unstaged change in the file it fails with
  `error: f: does not match index` (exit 1), writing nothing. VERIFIED.
- The general route is two writes: `apply --cached -R` then `apply -R` of the same
  patch. VERIFIED working; **not atomic**: if the worktree half fails (the user's
  unstaged edits overlap the hunk) the index half has already happened. `--check`
  both first narrows but does not close the window.

## 3. File-level stage, unstage, discard

All VERIFIED on 2.30 and 2.56 unless marked.

| Intent | Command | Notes |
| --- | --- | --- |
| Stage modified / new | `git add -- <path>` | Runs clean filter; `post-index-change`; takes `index.lock` |
| Stage a deletion | `git add -- <path>` or `git add -A -- <path>` | Both give `D ` for a file deleted from the worktree (git ≥ 2.0 semantics) |
| Mark intent | `git add -N -- <path>` | Status `.A`; `git diff --cached` ignores it, `git diff` shows it whole |
| Unstage | `git reset -q -- <path>` | **Works on an unborn branch** (`?? a.txt` after); `post-index-change 0 1` |
| Unstage (2.23+) | `git restore --staged -- <path>` | **Fails on unborn**: `fatal: could not resolve HEAD` (exit 128); runs `post-checkout <old> <new> 0` |
| Unstage on unborn | `git rm -q --cached -- <path>` | Works when index == worktree; **refuses** (`staged content different from both the file and the HEAD … use -f`) otherwise |
| Discard unstaged (to index) | `git checkout -- <path>` / `git restore -- <path>` | Takes `index.lock`; runs `post-checkout … 0` (both!) and smudge |
| Discard staged + unstaged (to HEAD) | `git restore --staged --worktree --source=HEAD -- <path>` or `git checkout HEAD -- <path>` | See next row |
| Discard a staged NEW file | `git restore --staged --worktree --source=HEAD -- <path>` | **Deletes the worktree file** (exit 0, `ls: cannot access`); `git checkout HEAD -- <path>` instead fails (`did not match any file(s) known to git`, exit 1) |

**Renames in the index:** after `git mv b.txt b2.txt` status shows `R  b.txt -> b2.txt`;
`git reset -- b2.txt` unstages only the new side (`D  b.txt` + `?? b2.txt`), and the
deletion side needs its own `reset -- b.txt`. Unstaging a staged rename means
naming both paths. VERIFIED.

**Submodules (VERIFIED):** `git add -- sub` stages the submodule's new commit
(`M  sub`); dirty content inside the submodule cannot be staged from the
superproject (` M sub` remains). `git checkout -- sub` and `git restore -- sub`
exit 0 and **do nothing** to the submodule's checked-out commit; only
`git submodule update -- sub` moves it back. `git clean` does not recurse into a
submodule. `git add` of a nested repository that is not a submodule stages a
gitlink with the `adding embedded git repository` warning (exit 0).

(An incidental finding: piping `git add` into `head -3` killed it with SIGPIPE
before it wrote the index — a reader of git's stderr that closes early aborts the
write.)

**Pathspec magic (VERIFIED).** A file literally named `:(top)x`:

```
$ git add -- ':(top)x'
fatal: pathspec ':(top)x' did not match any files        exit=128
$ GIT_LITERAL_PATHSPECS=1 git add -- ':(top)x'            exit=0, staged
$ git --literal-pathspecs add -- ':(top)x'               (same)
$ git add -- ':(literal):(top)x'                          exit=0, staged
$ GIT_LITERAL_PATHSPECS=1 git add -- '*'
fatal: pathspec '*' did not match any files               (no globbing)
```

Without literal mode `star*` is a glob and would also match `starfish`.
`GIT_LITERAL_PATHSPECS=1` (or `--literal-pathspecs`) is the safe default for every
path Cairn passes. DOCUMENTED (`git(1)`, `GIT_LITERAL_PATHSPECS`).

**Many paths (VERIFIED on 2.30):** `printf 'a\0b\0' | git <verb> --pathspec-from-file=- --pathspec-file-nul`
works for `add`, `reset`, `restore --staged`, `restore --worktree` and `checkout`,
and combines with `GIT_LITERAL_PATHSPECS=1`. **`git clean` has no such option**
(`error: unknown option 'pathspec-from-file=-'`, exit 129, on 2.30 and 2.56).
DOCUMENTED gates: 2.25 "A few commands learned to take the pathspec from the
standard input or a named file … with the '--pathspec-from-file' option"; 2.26
"'git rm' and 'git stash' learns the new '--pathspec-from-file' option". `restore`
itself is 2.23 (DOCUMENTED: "Two new commands 'git switch' and 'git restore' are
introduced"). All inside the floor.

## 4. Clean

`git clean -f [-d] [-x | -X] [-e <pattern>] -- <paths>`; dry run `-n`.

VERIFIED:

- Without `-f`: refuses, exit 128 (2.30: `clean.requireForce defaults to true and
  neither -i, -n, nor -f given; refusing to clean`; 2.56: `clean.requireForce is
  true and -f not given: refusing to clean`).
- `-n` lists untracked files only; `-nd` adds untracked directories (`ud/`,
  `emptydir/`) as one entry each; `-ndx` adds ignored ones; `-ndX` lists only
  ignored ones.
- **A pathspec naming a directory removes it without `-d`** (`git clean -n -- only`
  → `Would remove only/`). DOCUMENTED (`git-clean -d`): "If a <pathspec> is
  specified, -d is irrelevant; all untracked files matching the specified paths …
  will be removed."
- **Nested repositories:** `-fd` keeps an untracked directory holding a `.git`;
  `-ffd` removes it, its `.git` and every commit only it had. DOCUMENTED
  (`git-clean -f`): "Git will refuse to modify untracked nested git repositories …
  unless a second -f is given."
- Takes no `index.lock` (succeeded with one held); runs no hook.
- **`-n` output is not machine-readable:** no `-z`; names are C-quoted under
  `core.quotePath` (`"tab\tname"`, `"\303\274n\303\257"`), and the prefix
  `Would remove %s` is a translated message (its msgid is in
  `/usr/share/locale/de/LC_MESSAGES/git.mo`; the translation was not exercised
  because no German locale is generated here — UNVERIFIED that it prints
  translated). A confirmation list should come from a read that is parseable —
  `git ls-files -o --exclude-standard --directory -z` (VERIFIED: `only/\0zz1\0`)
  or the `-z` status Cairn already reads — and the clean run with exactly those
  literal paths.
- **What is unrecoverable:** everything it removes. Untracked and ignored files
  were never hashed; no reflog, no object, no trash.

## 5. Commit

`git commit -q --cleanup=<mode> -F -` with the message on stdin (`--file=-` is the
same). VERIFIED that `-F -` reads stdin and that **hooks get an empty stdin**, not
the message (`stdin=` in every hook's log).

**Cleanup (VERIFIED, matches DOCUMENTED `git-commit --cleanup`: "default — Same as
strip if the message is to be edited. Otherwise whitespace.").** With `-F` there is
no editor, so the default is `whitespace`: trailing spaces and trailing blank lines
go, **`#` lines stay**:

```
$ printf 'Subject line\n\n# not a comment to git with -F\nbody  \n\n\n' | git commit -q -F -
Subject line$
$
# not a comment to git with -F$
body$
```

`--cleanup=strip` drops `#` lines; `verbatim` keeps everything; `scissors` with
`-F` behaves as `whitespace` (the scissors line and what follows were kept —
DOCUMENTED: truncation applies "if the message is to be edited");
`core.commentChar=;` makes `strip` drop `;` lines and keep `#` ones.
`core.commentChar=auto` with `--cleanup=strip`: **2.30 kept `# hash line`, 2.56
dropped it**, and 2.56 warns `Support for 'core.commentChar=auto' is deprecated and
will be removed in Git 3.0`. Multi-character `core.commentChar` (2.45, DOCUMENTED:
"has been updated to allow an arbitrary multi-byte sequence") makes 2.30 and 2.32
refuse the commit (`error: core.commentChar should only be one character`).
`commit.template` is ignored with `-F` (VERIFIED). The message Cairn shows as
"what will be committed" must therefore apply the cleanup mode git will apply.

**Refusals (VERIFIED, exit 1 unless noted):**

- No message source: `error: There was a problem with the editor 'false'.` (2.56:
  `there was a problem with the editor 'false'`) — the `GIT_EDITOR=false` pin
  working.
- Empty after cleanup: `Aborting commit due to empty commit message.`
- Nothing staged: a `git status`-like text on **stdout** (`nothing to commit,
  working tree clean` / `nothing added to commit but untracked files present` /
  `no changes added to commit`), exit 1. `--allow-empty` overrides.
- Unmerged paths: `error: Committing is not possible because you have unmerged
  files.` … `fatal: Exiting because of an unresolved conflict.` (exit 128).

**Hooks (VERIFIED, `s5b.sh`, `s11.sh`).** Order: `pre-commit`,
`prepare-commit-msg .git/COMMIT_EDITMSG message` (`commit HEAD` for
`--amend --no-edit`), `commit-msg .git/COMMIT_EDITMSG`, `post-commit`; plus
`post-index-change` and `reference-transaction`, and `post-rewrite amend` after an
amend. Every hook saw `GIT_EDITOR=:` — **git overrides Cairn's `false` with `:`
for hooks when it is not launching an editor** — and `GIT_INDEX_FILE=.git/index`.
`--no-verify` skips `pre-commit` and `commit-msg` only (`prepare-commit-msg` and
`post-commit` still ran). A `commit-msg` hook that edits `$1` changes the stored
message. A `pre-commit` hook that `git add`s a formatted file gets that change into
the commit. A failing `pre-commit` (exit 3) fails the commit with exit 1 and the
hook's stderr; **the message is not saved anywhere by git** (pre-commit runs before
`COMMIT_EDITMSG` is written — the file held the previous commit's message), so the
draft is Cairn's to keep.

**Hooks and terminals (VERIFIED).** Without a controlling terminal:
`.git/hooks/pre-commit: line 2: /dev/tty: No such device or address` and the hook
continues. With one (git run under `script(1)`, started with `process_group=0`):

```
hook: opening tty
TIMEOUT: git state T ['    PID STAT CMD', '3744988 T<   /bin/sh .git/hooks/pre-commit']
```

The hook is stopped by SIGTTIN and never resumes; only a kill ends it. After the
group was SIGKILLed no `index.lock` remained and the staged change was intact.

**Signing.**

- `gpg.format=ssh` (2.34, DOCUMENTED: "ssh public crypto can be used for object
  and push-cert signing"): on **2.30 and 2.32 the config value itself is fatal** to
  commit: `error: unsupported value for gpg.format: ssh` /
  `fatal: bad config variable 'gpg.format' in file '.git/config' at line 7`
  (exit 128). VERIFIED.
- 2.56, unencrypted ssh key: `-S` signs (`BEGIN SSH SIGNATURE` in the object).
  Encrypted key, no agent, no controlling tty: **`ssh-keygen` asked `SSH_ASKPASS`**
  (`ASKPASS called with prompt: [Enter passphrase for ".../keys/pass": ]`) and the
  signed commit was made. With a refusing askpass: `incorrect passphrase supplied
  to decrypt private key?` / `fatal: failed to write commit object`, exit 128,
  nothing committed, index intact — **after pre-commit and commit-msg had already
  run**. VERIFIED.
- GPG: an unprotected key signed with no tty and no `DISPLAY` (`%G?` = `G`).
  A passphrase-protected key, with `DISPLAY`, `WAYLAND_DISPLAY` and `GPG_TTY`
  removed from git's environment, reached the user's already-running,
  systemd-socket-activated `gpg-agent`, which launched **its own** pinentry
  (`PINENTRY_LAUNCHED … gnome3`) and then failed with `gpg: signing failed:
  Timeout` / `fatal: failed to write commit object` (exit 128). So with a
  session agent the pinentry follows the agent's environment, not git's; what
  happens when git's gpg must autostart an agent with no display was not tested
  (UNVERIFIED). Cairn's inherited roster (`crates/cairn-git/src/process/environment.rs`)
  carries neither `GNUPGHOME` nor `GPG_TTY` nor a display variable — its own
  comment names `DISPLAY`, `WAYLAND_DISPLAY` and `GNUPGHOME` as open questions on
  issue #18. (See the hazard note on this experiment's side effects below.)

**Sign-off (VERIFIED).** `-s` appends `Signed-off-by: <user.name> <user.email>`
after a blank line and does not duplicate an identical trailer already last.
`format.signOff=true` has no effect on `commit` (it is `format-patch`'s).
`--trailer` is 2.32 (VERIFIED: `unknown option 'trailer'` on 2.30, works on 2.32;
DOCUMENTED: "'git commit' learned '--trailer <key>[=<value>]' option").

**Identity (VERIFIED).** With no `user.email` and a host without a domain:

```
Author identity unknown
*** Please tell me who you are.
...
fatal: unable to auto-detect email address (got '<user>@<host>.(none)')   exit=128
```

With `user.useConfigOnly=true`: `fatal: no email was given and auto-detection is
disabled`. With only `user.email` set, git **guessed the name from the passwd
GECOS field** and committed. On a host whose name has a domain, git would guess the
email too and commit silently under `-q` (UNVERIFIED here; the host has none).
`git var GIT_AUTHOR_IDENT` / `GIT_COMMITTER_IDENT` fails with the same text and
exit 128 when the identity is missing and prints `Name <email> <ts> <tz>` when it
is not — a read-only plumbing check usable before the commit.

**`i18n.commitEncoding` (VERIFIED).** With `ISO-8859-1` configured, a UTF-8 message
on stdin is stored byte-for-byte under `encoding ISO-8859-1`, and `git log
--format=%s` prints `caf\303\203\302\251` (double-encoded). With no setting, a
Latin-1 message is converted to UTF-8 with `Warning: commit message did not
conform to UTF-8.` Git expects the message in the configured encoding.

**Branch states (VERIFIED).** Unborn branch: the first commit works, reflog
`commit (initial): Subject line`. Detached HEAD: commit works (reflog `commit:
detached`), and the commit is reachable only from `HEAD` and its reflog once
`HEAD` moves. Merge in progress (`MERGE_HEAD`): `-F -` replaces `MERGE_MSG`, the
commit gets both parents, reflog `commit (merge): my merge message`, `MERGE_HEAD`
is removed. Rebase in progress: not tested (UNVERIFIED).

**Locks (VERIFIED).** `index.lock` held → exit 128, nothing written. `HEAD.lock` or
`refs/heads/main.lock` held → `fatal: cannot lock ref 'HEAD': Unable to create
'.../HEAD.lock'` exit 128 — after the hooks ran and the commit object was written,
so a dangling commit is left; the index stays staged. Ref locks are retried
`core.filesRefLockTimeout` (default 100 ms); the index lock is not retried at all
(DOCUMENTED, `git-config`; there is no index-lock timeout setting).

## 6. Amend

`git commit -q --amend [--no-edit | -F -] [--reset-author]`. All VERIFIED.

- Reflog (HEAD and branch): `commit (amend): <subject>`. Runs `post-rewrite amend`
  besides the commit hooks.
- `-F -` with `--no-edit`: `-F` wins.
- Nothing changed, `--no-edit`: a new commit with a new committer date (the oid
  only repeats within the same second); still a reflog entry.
- Amend that would make the commit's tree equal its parent's: refused, exit 1:
  `You asked to amend the most recent commit, but doing so would make it empty.
  You can repeat your command with --allow-empty, or you can remove the commit
  entirely with "git reset HEAD^".`
- `--reset-author` takes author name, email and date from the current identity;
  a plain amend keeps the author and changes only the committer.
- Root commit: amends fine (`parents=[]`).
- Merge commit: keeps both parents; reflog `commit (amend)`.
- Unborn: `fatal: You have nothing to amend.` (128).
- During a merge (`MERGE_HEAD`): `fatal: You are in the middle of a merge -- cannot
  amend.` (128).
- Staged changes are folded in.
- **Already pushed?** `git for-each-ref --contains HEAD --format='%(refname)' refs/remotes`
  prints `refs/remotes/origin/main` when the tip is on a remote-tracking ref and
  nothing after a local commit (2.30 and 2.56). After the amend the branch reads
  `[ahead 1, behind 1]`. This knows only what was last fetched.
- The replaced commit is reachable only from the reflogs (`HEAD@{1}`, `main@{1}`).

## 7. Stash

| Verb | VERIFIED behaviour |
| --- | --- |
| `git stash push -m <msg>` | Reflog of `refs/stash` gets `On main: <msg>`; HEAD's reflog gets `reset: moving to HEAD` |
| `-u` / `--include-untracked` | Third parent `untracked files on main: …` holding only the untracked files |
| `--keep-index` | Staged changes stay staged and in the worktree; the stash records both |
| `-- <pathspec>` (2.13) | Only those paths stashed; a pathspec matching nothing fails (`error: pathspec ':(,prefix:0)doesnotexist' did not match`, exit 1 on 2.30, 128 on 2.56) |
| `--pathspec-from-file=- --pathspec-file-nul` (2.26) | Works on 2.30 |
| `--staged` (2.35) | **2.30: `unknown option 'staged'` (129)**; 2.56 stashes the index only |
| No changes | `No local changes to save`, **exit 0**, nothing stored |
| Unborn branch | `You do not have the initial commit yet`, exit 1 (2.30) / 128 (2.56) |
| An ITA entry in the index | `error: Entry 'n' not uptodate. Cannot merge.`, exit 1 / 128, nothing changed |
| `apply --index` | Restores staged vs unstaged; without `--index` everything comes back unstaged |
| `apply` / `pop` by oid | `git stash apply <oid>` works; `git stash show <oid>` works |
| `drop <oid>` | **Refused**: `error: '<oid>' is not a stash reference` (exit 1 / 128) |
| `drop stash@{n}` | `Dropped stash@{1} (<oid>)`; later entries renumber |
| `drop stash@{9}` (out of range) | `fatal: log for 'stash' only has 3 entries` (128) |
| `store -m <msg> <oid>` | Puts a dropped stash commit back at `stash@{0}` |

Gates DOCUMENTED: 2.13 "'git stash push' takes a pathspec so that the local changes
can be stashed away only partially"; 2.26 `--pathspec-from-file`; 2.35 "'git stash'
learned the '--staged' option". (DOCUMENTED, `git-stash`: `<stash>` is "A reference
of the form stash@{<revision>}".)

**Conflicts (VERIFIED).** `pop` onto a changed base: `CONFLICT (content)` … `The
stash entry is kept in case you need it again.`, exit 1, index `UU`. `pop` over a
dirty file the stash touches: `error: Your local changes to the following files
would be overwritten by merge` … `Aborting` … entry kept, exit 1 (2.30) / 128
(2.56). An untracked file in the way of the stash's untracked part: `u already
exists, no checkout` / `error: could not restore untracked files from stash`,
the existing file untouched, exit 1 / 128. `apply --index` whose index part does
not apply: `error: conflicts in index. Try without --index.` (exit 1 / 128).
2.35 fixed "'git stash apply' forgot to attempt restoring untracked files when it
failed to restore changes to tracked ones" (DOCUMENTED) — so a floor git behaves
differently on a mixed failure (UNVERIFIED which way).

**The list is the reflog (VERIFIED).** `.git/logs/refs/stash` holds one line per
entry (`… Test User <test@example.com> 1791396825 +0100	On main: msg one`);
dropping the last entry deletes `refs/stash` and its log (`git reflog exists
refs/stash` → exit 1).

**Recovery after drop (VERIFIED).** The dropped commit appears in no reflog
(`git log -g --all --format=%H | grep -c <oid>` → `0`); `git fsck --no-reflogs`
reports `dangling commit <oid>`; `git stash store` restores it. It survives until
`gc` prunes unreachable objects (`gc.pruneExpire`, default 2 weeks, DOCUMENTED). The
`git-stash` man page's own recovery recipe is `git fsck --unreachable | grep commit
| … | xargs git log --merges --no-walk --grep=WIP`.

**Stash entries never expire by default (VERIFIED).** Entries dated 10, 40, 60 and
120 days ago all survived `git reflog expire --all` and `git gc` on 2.30 and 2.56,
although `gc.reflogExpire` is 90 days and `gc.reflogExpireUnreachable` 30
(DOCUMENTED defaults). Git special-cases `refs/stash` unless
`gc.refs/stash.reflogExpire` is configured (the pattern form is DOCUMENTED; the
special case is from git's `reflog` source as remembered — UNVERIFIED by reading).

**Atomicity (VERIFIED, `s10b.sh`).** One `stash push` wrote the index six times
(`post-index-change` ran six times) and committed five ref transactions on 2.56.
Killed 1.5 s in, during the slow smudge of its `reset --hard`:

```
Saved working directory and index state On main: killed
KILLED group
stash list: stash@{0}: On main: killed
.git/index.lock                      <- stale
cat: f.s: No such file or directory  <- deleted mid-smudge
z                                    <- g still has its change (not yet reset)
```

`git stash show -p stash@{0}` held both changes; after removing the lock,
`git reset --hard && git stash pop --index` restored both. Where the kill lands
before the entry is stored (while it builds the commits), the worktree is
untouched and only unreferenced objects are left (UNVERIFIED by a kill at that
point; inferred from the order the transcript shows).

## 8. Reflog

**Reading (VERIFIED).** `git reflog` = `git log -g --abbrev-commit --oneline` style;
a parseable form:

```
git log -g -z --format='%H%x1f%gD%x1f%gs%x1f%gn%x1f%ct%x1f%p' HEAD
b10f3633…|HEAD@{0}|cherry-pick: second-amended|Test User|1791396950|d79b77c
```

`git rev-list -g` walks the same. There is no plumbing that prints a reflog
record whole; the old oid of each entry is not available through `--format` and
only the raw log (`.git/logs/<ref>`: `<old> <new> <ident> <ts> <tz>\t<msg>`) has it.
`%gd`/`%gD` switch to the date form (`HEAD@{2026-10-07 19:15:50 +0100}`) when
`--date` is given, so a reader wanting `@{n}` must not pass one. `git reflog list`
(enumerate reflogs) is 2.45 (DOCUMENTED); `git reflog exists <ref>` is on the floor.

**What writes there (VERIFIED, HEAD log):** `commit (initial): …`, `commit: …`,
`commit (amend): …`, `commit (merge): …`, `checkout: moving from feat to main`,
`reset: moving to HEAD~1`, `cherry-pick: …`, and `git stash push` and `git merge
--abort` each add `reset: moving to HEAD`. Stash push/drop write the `refs/stash`
log, not HEAD's. Branch logs (`logs/refs/heads/<b>`) record the same commits but
not checkouts.

**`core.logAllRefUpdates` (VERIFIED + DOCUMENTED).** `git init` writes
`core.logAllRefUpdates = true` into a non-bare repository's config; a bare one has
none (default false). With it false, a new branch gets no log while existing logs
keep growing ("only when the file exists", DOCUMENTED).

**Reftable (VERIFIED, 2.56).** `git init --ref-format=reftable` (2.45, DOCUMENTED)
gives a repository with **no `.git/logs/`** at all; `git reflog` still works. The
vendored `gix-ref-0.67.1` source contains no mention of reftable (grep), so a gix
reflog reader sees nothing there (UNVERIFIED beyond the grep).

**Recovery from an entry** is ordinary writes — `git branch <name> <oid>` (refuses an
existing name), `git stash apply <oid>` (VERIFIED works by oid), `git reset --hard
<oid>` (destructive: discards the worktree) — none of which needs the reflog to be
writable.

## 9. `.gitignore` editing

A file write, not a git verb. VERIFIED:

- **Which file decided:** `git check-ignore -v [-z] [--stdin] [--non-matching]
  --no-index -- <path>` prints `source:line:pattern<TAB>path`. A nearer
  `.gitignore` outranks `.git/info/exclude`, which outranks `core.excludesFile`
  (`sub/.gitignore:1:*.log` reported over `info/exclude`'s `y.log` and the global
  `x.log`).
- `--no-index` is needed for a tracked path: without it a tracked file matching a
  pattern exits 1 (not ignored, because tracked); with it the pattern is shown.
- A negation cannot re-include a file under an ignored directory
  (`build/` + `!build/keep` → `build/keep` reported ignored by `build/`).
- **Escapes that work** (each name matched exactly, `abc` and `starfish` not):
  `\#hash`, `\!bang`, `trail\ ` (escaped trailing space), `a\[b\]c`, `star\*`,
  `back\\slash`; an inner space needs no escape (`sp ace`). An unescaped trailing
  space is stripped (`trail ` then ignores `trail`, not `trail `).
- **Appending to a file with no final newline joins lines** (`abc` + `starfish` →
  `abcstarfish`, ignoring neither).
- Anchoring with a leading `/` limits a pattern to its own directory (DOCUMENTED,
  gitignore(5)); a path containing a newline cannot be written as a pattern
  (UNVERIFIED: gitignore has no newline escape).

## 10. Locking and concurrency

VERIFIED (§1, §5, `s10.sh`, `s10c.sh`):

| Verb | Needs `index.lock` | Under a held lock |
| --- | --- | --- |
| `apply --cached`, `add`, `reset -- p`, `restore --staged`, `checkout -- p`, `commit` | yes | exit 128, nothing written |
| `stash push` | yes | 2.30: exit 1, silent under `-q`; 2.56: `error: … index.lock … could not write index`, exit 128 |
| `apply` (worktree), `clean` | **no** | succeed |
| `status` | optional | succeeds (refresh skipped) |

The 2.30 message is a paragraph (`Another git process seems to be running in this
repository, e.g. an editor opened by 'git commit'. … remove the file manually to
continue.`); 2.56 says `Another git process seems to be running in this repository,
or the lock file may be stale`. Recent git can write a PID file beside a lock
(`core.lockfilePid`, default false, DOCUMENTED in 2.56's `git-config`; not in the
floor, version UNVERIFIED). Any IDE or terminal `git` holding the lock for an
instant makes a Cairn write fail with 128; only a `git status` without
`GIT_OPTIONAL_LOCKS=0` takes it for a refresh (DOCUMENTED, `git(1)`
`GIT_OPTIONAL_LOCKS`).

**SIGKILL mid-write (VERIFIED, process group killed):**

| Killed during | Left behind |
| --- | --- |
| `git add` in a slow clean filter | **stale `index.lock`**; index unchanged |
| `git commit` in `pre-commit` | no lock; nothing committed; staged state intact |
| `git commit` in `commit-msg` | no lock; nothing committed; `COMMIT_EDITMSG` holds the message |
| `git commit` in `post-commit` | commit made; no lock |
| `git stash push` in its reset | entry stored; worktree half-reset; **stale `index.lock`** (§7) |

So cancelling an index write can leave a lock nothing will remove; the next write
fails until it is deleted. Whether a lock is stale cannot be told from the file on
the floor (no PID file).

## 11. What to re-read after each verb

From what each verb was seen to write (hooks counted, `s11.sh`):

| Verb | Index | Worktree | HEAD / branch | `refs/stash` | Reflog |
| --- | --- | --- | --- | --- | --- |
| `apply --cached [-R]` | yes | — | — | — | — |
| `apply [-R]` (worktree) | — | yes | — | — | — |
| `add`, `reset -- p`, `restore --staged` | yes | — (clean filter only reads) | — | — | — |
| `checkout -- p`, `restore [--worktree]` | yes (stat) | yes | — (`post-checkout` runs) | — | — |
| `clean` | — | yes | — | — | — |
| `commit`, `commit --amend` | yes (hooks may `git add`; cache-tree) | maybe (a `pre-commit` formatter) | yes | — | HEAD + branch |
| `stash push` | yes | yes | — | yes | `refs/stash` + HEAD (`reset: moving to HEAD`) |
| `stash apply/pop` | yes | yes | — | pop: yes | pop: `refs/stash` |
| `stash drop/store` | — | — | — | yes | `refs/stash` |

A commit can also start detached `gc --auto`/maintenance (DOCUMENTED, `git-gc`;
`pre-auto-gc` was armed but did not fire in these small repositories), which
repacks and can prune objects past their grace period. Any verb whose hooks run
user code can change anything; a full status re-read after every write is the
only safe rule.

## Hazards and open questions for the packet

1. **Patch form.** Every patch Cairn emits for `apply` must be in git's form
   against the exact index blob (for `--cached`) or git's form of the worktree
   file (worktree). A CRLF or smudged patch fails; a patch built from gix's own
   read of a filtered file may not be git's form.
2. **Pin `--whitespace=nowarn` on every `apply`.** Otherwise `apply.whitespace=fix`
   stages content the user never had, and `=error` fails staging. Whether to also
   pin `apply.ignoreWhitespace=false` (only via `-c` on the floor) is a decision;
   its effect is unverified.
3. **Non-atomic pairs.** Discarding a staged hunk (`--cached -R` then `-R`) and
   unstaging a rename (two paths) are two writes; the first can succeed and the
   second fail. `apply --index -R` is atomic but refuses whenever the file has
   other unstaged changes.
4. **Discards are unrecoverable.** Worktree `apply -R`, `checkout -- p`,
   `restore --worktree`, `clean` (and `-ffd` on a nested repository, which takes
   its unpushed history) destroy data no object holds. `restore --staged
   --worktree --source=HEAD` on a staged new file deletes the file — its blob
   survives only as an unreachable object.
5. **Stash.** Exit 0 when nothing was stashed; drop only by `stash@{n}` (race with
   a concurrent push — check the oid immediately before and accept the window, or
   design around it); dropped entries recoverable only via `fsck`; ITA entries
   break push; `--staged` absent below 2.35; push is non-atomic and a kill leaves
   a stale lock. Exit codes differ 1 vs 128 between 2.30 and 2.56 for the same
   failure.
6. **Stale `index.lock` after a cancel.** A killed `add` or `stash push` leaves one;
   every later write fails with 128 until it is removed. Who removes it, and how a
   user is told it is Cairn's own (vs another live process's), is open — the floor
   has no PID file.
7. **Hooks.** A hook reading `/dev/tty` hangs forever when Cairn was launched from
   a terminal (process group, not session); `setsid`-style detachment would turn
   that into an ENXIO the hook can handle. Hooks see `GIT_EDITOR=:`, not `false`.
   A failed `pre-commit` leaves the message nowhere — Cairn must keep the draft.
   Hook output (often the only explanation of a refusal) arrives on stderr and
   must be shown. `post-checkout` runs on file-level discard and restore.
8. **Signing.** Commit needs an askpass token for ssh signing with an encrypted key.
   `gpg.format=ssh` in config breaks every commit on gits below 2.34. GPG pinentry
   depends on the agent's environment (`DISPLAY`/`GPG_TTY`/`GNUPGHOME` are not on
   Cairn's roster — issue #18); behaviour when Cairn's git must autostart the agent
   is unverified. Signing fails after hooks have run.
9. **Message fidelity.** Show and send the message as git will clean it
   (`whitespace` under `-F` keeps `#` lines; `commit.cleanup` and
   `core.commentChar` are the user's, and `auto` behaves differently on 2.30 and
   2.56). Transcode to `i18n.commitEncoding` or the commit is mojibake.
10. **Identity.** Check `git var GIT_AUTHOR_IDENT` before committing; git may
    silently guess a name (and on some hosts an email).
11. **Pathspecs.** Run every path-taking verb with `GIT_LITERAL_PATHSPECS=1` (or
    `--literal-pathspecs`) and `--pathspec-from-file=- --pathspec-file-nul` where
    many paths go; `clean` has no pathspec file, so many paths go on argv after
    `--` (argv length limits UNVERIFIED for very large selections).
12. **`clean -n` is not a confirmation source.** It is localized and quoted; build
    the list from a `-z` read and clean exactly those paths — and accept the TOCTOU
    window between the confirm and the run.
13. **Unborn branch.** Unstage with `reset -- p` (works) not `restore --staged`
    (fails); stash and amend refuse.
14. **Reflog.** No plumbing prints whole records; `.git/logs` does not exist under
    reftable (2.45+), and gix 0.87's ref crate appears not to read reftable.
    Stash entries never expire by default.
15. **Open:** commit during a rebase in progress; `apply.ignoreWhitespace`'s
    effect; a kill of `stash push` before it stores; the floor's behaviour on a
    mixed tracked/untracked stash-apply failure (fixed in 2.35).

### Side effect of this research (for the user)

The GPG experiment set `HOME` to a scratch directory whose `.gnupg` resolved to the
**same agent socket as the real session agent** (`$XDG_RUNTIME_DIR/gnupg/S.gpg-agent`),
so the session's systemd-activated `gpg-agent` handled the scratch key generation:
two throwaway test private keys were written to the real
`~/.gnupg/private-keys-v1.d/`, a pinentry may have appeared on the desktop and timed
out, and `gpgconf --kill gpg-agent` restarted the session agent (cached passphrases
dropped). The user was told and given the commands to remove the keys (their
keygrips and passphrase are deliberately not recorded here). Lesson for any later
experiment: a scratch `HOME` does not isolate gpg — set `GNUPGHOME` to a scratch
directory and start a private agent, or do not run gpg at all.
